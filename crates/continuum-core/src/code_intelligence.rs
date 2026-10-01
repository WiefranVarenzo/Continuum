use std::collections::{BTreeSet, HashMap, HashSet};
use std::path::Path;
use std::time::Instant;

use ast_grep_language::{LanguageExt, SupportLang};
use chrono::Utc;
use rusqlite::{Connection, OptionalExtension, Transaction, TransactionBehavior, params};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::development::{
    GitTreeFile, insert_common_entity, insert_development_timeline, read_repository_tree_files,
    reject_ai_actor, require_development_enabled, upsert_development_search,
};
use crate::store::{
    append_event_with_context, bounded_json, prior_result, record_command_with_context,
    validate_command_context, validate_nonempty,
};
use crate::{
    CommandContext, ContinuityStore, CoreError, Entity, IntegrityIssue, IntegrityReport,
    OriginKind, RepositoryBaselineRecord, Result, new_id,
};

const ANALYZER_BUNDLE_VERSION: &str = "continuum-code-intelligence-v1";
const AST_GREP_VERSION: &str = "ast-grep-language-0.45.3-rules-v1";
const MANIFEST_ANALYZER_VERSION: &str = "continuum-manifest-v1";
const FALLBACK_ANALYZER_VERSION: &str = "continuum-file-fallback-v1";
const FILE_ANALYSIS_CACHE_VERSION: &str = "ast-grep-language-0.45.3-rules-v1+continuum-manifest-v1";
const ANALYZER_CONTRACT_VERSION: i64 = 1;
const OUTPUT_SCHEMA_VERSION: i64 = 1;
const MAX_FILES_HARD: usize = 50_000;
const MAX_FILE_BYTES_HARD: usize = 8 * 1024 * 1024;
const MAX_TOTAL_BYTES_HARD: usize = 512 * 1024 * 1024;
const MAX_ENTITIES_HARD: usize = 200_000;
const MAX_TEST_RESULTS: usize = 20_000;
const MAX_JSON_BYTES: usize = 1024 * 1024;
const ANALYZER_CACHE_MAX_BYTES: i64 = 256 * 1024 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AnalysisLimits {
    pub max_files: usize,
    pub max_file_bytes: usize,
    pub max_total_bytes: usize,
    pub max_entities: usize,
}

impl Default for AnalysisLimits {
    fn default() -> Self {
        Self {
            max_files: 20_000,
            max_file_bytes: 2 * 1024 * 1024,
            max_total_bytes: 128 * 1024 * 1024,
            max_entities: 100_000,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AnalyzeBaselineInput {
    pub repository_id: String,
    pub baseline_id: String,
    #[serde(default)]
    pub limits: AnalysisLimits,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AnalyzerExecutionRecord {
    pub analyzer_id: String,
    pub analyzer_version: String,
    pub contract_version: i64,
    pub output_schema_version: i64,
    pub status: String,
    pub files_seen: usize,
    pub outputs_created: usize,
    pub cache_hits: usize,
    pub cache_misses: usize,
    pub duration_ms: u64,
    pub diagnostic_code: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AnalysisLimitationRecord {
    pub id: String,
    pub source_path: Option<String>,
    pub analyzer_id: String,
    pub code: String,
    pub severity: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AnalysisRunRecord {
    pub entity: Entity,
    pub repository_id: String,
    pub baseline_id: String,
    pub source_fingerprint: String,
    pub analyzer_bundle_version: String,
    pub output_schema_version: i64,
    pub completeness: String,
    pub file_count: usize,
    pub analyzed_file_count: usize,
    pub code_entity_count: usize,
    pub test_count: usize,
    pub limitation_count: usize,
    pub cache_hit_count: usize,
    pub cache_miss_count: usize,
    pub limits: AnalysisLimits,
    pub started_at: String,
    pub completed_at: String,
    pub duration_ms: u64,
    pub analyzer_executions: Vec<AnalyzerExecutionRecord>,
    pub limitations: Vec<AnalysisLimitationRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CodeEntityRecord {
    pub entity: Entity,
    pub repository_id: String,
    pub entity_kind: String,
    pub stable_key: String,
    pub language: Option<String>,
    pub first_seen_baseline_id: String,
    pub latest_observation: Option<CodeObservationRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CodeObservationRecord {
    pub id: String,
    pub analysis_run_id: String,
    pub baseline_id: String,
    pub code_entity_id: String,
    pub source_path: String,
    pub qualified_name: Option<String>,
    pub content_sha256: String,
    pub signature_sha256: String,
    pub start_line: Option<u64>,
    pub start_column: Option<u64>,
    pub end_line: Option<u64>,
    pub end_column: Option<u64>,
    pub visibility: Option<String>,
    pub observation_status: String,
    pub analyzer_id: String,
    pub analyzer_version: String,
    pub output_schema_version: i64,
    pub details: Value,
    pub observed_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TestRecord {
    pub entity: Entity,
    pub repository_id: String,
    pub stable_key: String,
    pub framework: String,
    pub test_kind: String,
    pub first_seen_baseline_id: String,
    pub latest_observation: Option<TestObservationRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TestObservationRecord {
    pub id: String,
    pub analysis_run_id: String,
    pub baseline_id: String,
    pub test_id: String,
    pub code_entity_id: String,
    pub source_path: String,
    pub qualified_name: String,
    pub content_sha256: String,
    pub start_line: u64,
    pub end_line: u64,
    pub analyzer_id: String,
    pub analyzer_version: String,
    pub details: Value,
    pub observed_at: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TestOutcome {
    Passed,
    Failed,
    Error,
    Cancelled,
    Skipped,
    Unknown,
}

impl TestOutcome {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Passed => "passed",
            Self::Failed => "failed",
            Self::Error => "error",
            Self::Cancelled => "cancelled",
            Self::Skipped => "skipped",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TestRunSource {
    User,
    Import,
    External,
    Legacy,
    Unknown,
}

impl TestRunSource {
    fn as_str(self) -> &'static str {
        match self {
            Self::User => "user",
            Self::Import => "import",
            Self::External => "external",
            Self::Legacy => "legacy",
            Self::Unknown => "unknown",
        }
    }

    fn origin(self) -> OriginKind {
        match self {
            Self::User => OriginKind::User,
            Self::Import => OriginKind::Import,
            Self::External => OriginKind::External,
            Self::Legacy => OriginKind::Legacy,
            Self::Unknown => OriginKind::Unknown,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TestResultInput {
    pub test_id: String,
    pub outcome: TestOutcome,
    pub duration_ms: Option<u64>,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NewTestRun {
    pub title: String,
    pub repository_id: String,
    pub baseline_id: String,
    pub outcome: TestOutcome,
    pub command_label: String,
    pub exit_code: Option<i32>,
    pub duration_ms: Option<u64>,
    pub source: TestRunSource,
    #[serde(default)]
    pub results: Vec<TestResultInput>,
    #[serde(default)]
    pub details: Value,
    #[serde(default)]
    pub metadata: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TestRunRecord {
    pub entity: Entity,
    pub repository_id: String,
    pub baseline_id: String,
    pub outcome: String,
    pub command_label: String,
    pub exit_code: Option<i32>,
    pub duration_ms: Option<u64>,
    pub observed_at: String,
    pub source_kind: String,
    pub details: Value,
    pub results: Vec<TestRunResultRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TestRunResultRecord {
    pub test_id: String,
    pub outcome: String,
    pub duration_ms: Option<u64>,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LinkTestVerificationInput {
    pub test_id: String,
    pub target_entity_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ParsedFileOutput {
    language: Option<String>,
    parser_had_error: bool,
    symbols: Vec<ParsedSymbol>,
    tests: Vec<ParsedTest>,
    dependencies: Vec<ParsedDependency>,
    configuration: Option<ParsedConfiguration>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ParsedSymbol {
    kind: String,
    name: String,
    qualified_name: String,
    start_line: u64,
    start_column: u64,
    end_line: u64,
    end_column: u64,
    visibility: Option<String>,
    content_sha256: String,
    signature_sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ParsedTest {
    name: String,
    qualified_name: String,
    framework: String,
    test_kind: String,
    start_line: u64,
    end_line: u64,
    content_sha256: String,
    symbol_qualified_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ParsedDependency {
    ecosystem: String,
    scope: String,
    name: String,
    requirement: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ParsedConfiguration {
    format: String,
    parse_status: String,
    top_level_keys: Vec<String>,
    sensitive_values_omitted: bool,
}

#[derive(Debug)]
struct PreparedFile {
    path: String,
    mode: String,
    object_type: String,
    object_id: String,
    content_sha256: String,
    output: ParsedFileOutput,
    status: String,
    analyzer_id: String,
    analyzer_version: String,
    limitations: Vec<PreparedLimitation>,
    cache_hit: bool,
}

#[derive(Debug, Clone)]
struct PreparedLimitation {
    path: Option<String>,
    analyzer_id: String,
    code: String,
    severity: String,
    message: String,
}

#[derive(Debug)]
struct PreparedAnalysis {
    files: Vec<PreparedFile>,
    limitations: Vec<PreparedLimitation>,
    cache_entries: Vec<(String, String, String)>,
    dirty_paths: BTreeSet<String>,
    cache_hits: usize,
    cache_misses: usize,
    analyzed_files: usize,
}

pub(crate) fn is_code_intelligence_entity_type(value: &str) -> bool {
    matches!(value, "analysis_run" | "code_entity" | "test" | "test_run")
}

impl ContinuityStore {
    pub fn analyze_repository_baseline(
        &self,
        command: &CommandContext,
        input: AnalyzeBaselineInput,
    ) -> Result<AnalysisRunRecord> {
        validate_command_context(command)?;
        reject_ai_actor(command)?;
        validate_analysis_limits(&input.limits)?;
        let repository = self.get_repository(&input.repository_id)?;
        let baseline = self.get_repository_baseline(&input.baseline_id)?;
        if baseline.repository_id != repository.entity.id {
            return Err(CoreError::Validation(
                "analysis baseline does not belong to the requested Repository".into(),
            ));
        }
        {
            let mut connection = self.connection()?;
            let tx = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
            require_development_enabled(&tx, &self.manifest().project_id)?;
            if let Some(Some(id)) = prior_result(
                &tx,
                &self.manifest().project_id,
                command,
                "AnalyzeRepositoryBaseline",
            )? {
                tx.commit()?;
                return self.get_analysis_run(&id);
            }
            tx.commit()?;
        }
        let source_fingerprint = analysis_source_fingerprint(&baseline);
        if let Some(id) = existing_analysis_run(
            &self.connection()?,
            &self.manifest().project_id,
            &input.repository_id,
            &input.baseline_id,
            &source_fingerprint,
        )? {
            let mut connection = self.connection()?;
            let tx = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
            return reapply_analysis_projection(
                self,
                tx,
                command,
                &id,
                &input.repository_id,
                &input.baseline_id,
            );
        }
        let started_at = Utc::now().to_rfc3339();
        let timer = Instant::now();
        let prepared = prepare_analysis(
            &self.connection()?,
            Path::new(&repository.root_path),
            &baseline,
            &input.limits,
        )?;
        let duration_ms = timer.elapsed().as_millis().min(u128::from(u64::MAX)) as u64;
        let completed_at = Utc::now().to_rfc3339();
        let mut connection = self.connection()?;
        let tx = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        require_development_enabled(&tx, &self.manifest().project_id)?;
        if let Some(Some(id)) = prior_result(
            &tx,
            &self.manifest().project_id,
            command,
            "AnalyzeRepositoryBaseline",
        )? {
            tx.commit()?;
            return self.get_analysis_run(&id);
        }
        if let Some(id) = existing_analysis_run(
            &tx,
            &self.manifest().project_id,
            &input.repository_id,
            &input.baseline_id,
            &source_fingerprint,
        )? {
            return reapply_analysis_projection(
                self,
                tx,
                command,
                &id,
                &input.repository_id,
                &input.baseline_id,
            );
        }
        persist_analysis(
            self,
            tx,
            command,
            &repository.entity.id,
            &baseline,
            &source_fingerprint,
            &input.limits,
            &started_at,
            &completed_at,
            duration_ms,
            prepared,
        )
    }

    pub fn get_analysis_run(&self, id: &str) -> Result<AnalysisRunRecord> {
        let entity = self.get_entity(id)?;
        if entity.entity_type != "analysis_run" {
            return Err(CoreError::Validation("entity is not an AnalysisRun".into()));
        }
        let connection = self.connection()?;
        let raw = connection
            .query_row(
                "SELECT repository_id,baseline_id,source_fingerprint,analyzer_bundle_version,
                        output_schema_version,completeness,file_count,analyzed_file_count,
                        code_entity_count,test_count,limitation_count,cache_hit_count,cache_miss_count,
                        limits_json,started_at,completed_at,duration_ms
                 FROM analysis_runs WHERE entity_id=?1 AND project_id=?2",
                params![id, self.manifest().project_id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?, row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?, row.get::<_, String>(3)?,
                        row.get::<_, i64>(4)?, row.get::<_, String>(5)?,
                        row.get::<_, i64>(6)?, row.get::<_, i64>(7)?,
                        row.get::<_, i64>(8)?, row.get::<_, i64>(9)?,
                        row.get::<_, i64>(10)?, row.get::<_, i64>(11)?,
                        row.get::<_, i64>(12)?, row.get::<_, String>(13)?,
                        row.get::<_, String>(14)?, row.get::<_, String>(15)?,
                        row.get::<_, i64>(16)?,
                    ))
                },
            )
            .optional()?
            .ok_or_else(|| CoreError::NotFound(id.into()))?;
        Ok(AnalysisRunRecord {
            entity,
            repository_id: raw.0,
            baseline_id: raw.1,
            source_fingerprint: raw.2,
            analyzer_bundle_version: raw.3,
            output_schema_version: raw.4,
            completeness: raw.5,
            file_count: raw.6 as usize,
            analyzed_file_count: raw.7 as usize,
            code_entity_count: raw.8 as usize,
            test_count: raw.9 as usize,
            limitation_count: raw.10 as usize,
            cache_hit_count: raw.11 as usize,
            cache_miss_count: raw.12 as usize,
            limits: serde_json::from_str(&raw.13)?,
            started_at: raw.14,
            completed_at: raw.15,
            duration_ms: raw.16 as u64,
            analyzer_executions: read_analyzer_executions(&connection, id)?,
            limitations: read_limitations(&connection, id)?,
        })
    }

    pub fn get_code_entity(&self, id: &str) -> Result<CodeEntityRecord> {
        let entity = self.get_entity(id)?;
        if entity.entity_type != "code_entity" {
            return Err(CoreError::Validation("entity is not a CodeEntity".into()));
        }
        let connection = self.connection()?;
        let raw = connection
            .query_row(
                "SELECT repository_id,entity_kind,stable_key,language,first_seen_baseline_id
                 FROM code_entities WHERE entity_id=?1 AND project_id=?2",
                params![id, self.manifest().project_id],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                    ))
                },
            )
            .optional()?
            .ok_or_else(|| CoreError::NotFound(id.into()))?;
        Ok(CodeEntityRecord {
            entity,
            repository_id: raw.0,
            entity_kind: raw.1,
            stable_key: raw.2,
            language: raw.3,
            first_seen_baseline_id: raw.4,
            latest_observation: latest_code_observation(&connection, id)?,
        })
    }

    pub fn get_test(&self, id: &str) -> Result<TestRecord> {
        let entity = self.get_entity(id)?;
        if entity.entity_type != "test" {
            return Err(CoreError::Validation("entity is not a Test".into()));
        }
        let connection = self.connection()?;
        let raw = connection
            .query_row(
                "SELECT repository_id,stable_key,framework,test_kind,first_seen_baseline_id
                 FROM tests WHERE entity_id=?1 AND project_id=?2",
                params![id, self.manifest().project_id],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                    ))
                },
            )
            .optional()?
            .ok_or_else(|| CoreError::NotFound(id.into()))?;
        Ok(TestRecord {
            entity,
            repository_id: raw.0,
            stable_key: raw.1,
            framework: raw.2,
            test_kind: raw.3,
            first_seen_baseline_id: raw.4,
            latest_observation: latest_test_observation(&connection, id)?,
        })
    }

    pub fn record_test_run(
        &self,
        command: &CommandContext,
        input: NewTestRun,
    ) -> Result<TestRunRecord> {
        validate_command_context(command)?;
        reject_ai_actor(command)?;
        validate_nonempty(&input.title, 500, "TestRun title")?;
        validate_nonempty(&input.command_label, 2_000, "TestRun command label")?;
        validate_json(&input.details, "TestRun details")?;
        validate_json(&input.metadata, "TestRun metadata")?;
        if input.results.len() > MAX_TEST_RESULTS {
            return Err(CoreError::Validation(format!(
                "TestRun may contain at most {MAX_TEST_RESULTS} results"
            )));
        }
        ensure_unique(
            input.results.iter().map(|result| result.test_id.as_str()),
            "TestRun result test IDs must be unique",
        )?;
        for result in &input.results {
            if let Some(message) = &result.message {
                validate_text(message, 10_000, "TestRun result message")?;
            }
        }
        validate_test_run_consistency(input.outcome, &input.results)?;
        let repository = self.get_repository(&input.repository_id)?;
        let baseline = self.get_repository_baseline(&input.baseline_id)?;
        if baseline.repository_id != repository.entity.id {
            return Err(CoreError::Validation(
                "TestRun baseline does not belong to the requested Repository".into(),
            ));
        }
        let details_json = bounded_json(&input.details, MAX_JSON_BYTES, "TestRun details")?;
        let metadata_json = bounded_json(&input.metadata, MAX_JSON_BYTES, "TestRun metadata")?;
        let mut connection = self.connection()?;
        let tx = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        require_development_enabled(&tx, &self.manifest().project_id)?;
        if let Some(Some(id)) =
            prior_result(&tx, &self.manifest().project_id, command, "RecordTestRun")?
        {
            tx.commit()?;
            return self.get_test_run(&id);
        }
        for result in &input.results {
            require_typed_entity(&tx, &self.manifest().project_id, &result.test_id, "test")?;
            let test_repository: String = tx.query_row(
                "SELECT repository_id FROM tests WHERE entity_id=?1",
                [&result.test_id],
                |row| row.get(0),
            )?;
            if test_repository != input.repository_id {
                return Err(CoreError::Validation(
                    "TestRun cannot contain a Test from another Repository".into(),
                ));
            }
        }
        let id = new_id();
        let now = Utc::now().to_rfc3339();
        insert_common_entity(
            &tx,
            &self.manifest().project_id,
            command,
            &id,
            "test_run",
            &input.title,
            input.outcome.as_str(),
            input.source.origin(),
            &metadata_json,
            &json!({"repository_id":input.repository_id,"baseline_id":input.baseline_id,
                "outcome":input.outcome.as_str(),"source_kind":input.source.as_str()}),
            &now,
        )?;
        tx.execute(
            "INSERT INTO test_runs(entity_id,project_id,repository_id,baseline_id,outcome,
                command_label,exit_code,duration_ms,observed_at,source_kind,details_json)
             VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)",
            params![
                id,
                self.manifest().project_id,
                input.repository_id,
                input.baseline_id,
                input.outcome.as_str(),
                input.command_label,
                input.exit_code,
                input.duration_ms.map(|value| value as i64),
                now,
                input.source.as_str(),
                details_json
            ],
        )?;
        for result in &input.results {
            tx.execute(
                "INSERT INTO test_run_results(test_run_id,test_id,outcome,duration_ms,message)
                 VALUES(?1,?2,?3,?4,?5)",
                params![
                    id,
                    result.test_id,
                    result.outcome.as_str(),
                    result.duration_ms.map(|value| value as i64),
                    result.message
                ],
            )?;
            insert_relationship_if_absent(
                &tx,
                &self.manifest().project_id,
                command,
                &id,
                "test_run",
                &result.test_id,
                "test",
                "executes",
                input.source.origin(),
                std::slice::from_ref(&input.baseline_id),
            )?;
        }
        insert_relationship_if_absent(
            &tx,
            &self.manifest().project_id,
            command,
            &id,
            "test_run",
            &input.baseline_id,
            "repository_baseline",
            "observed_at",
            input.source.origin(),
            std::slice::from_ref(&input.baseline_id),
        )?;
        upsert_development_search(
            &tx,
            &self.manifest().project_id,
            &id,
            "test_run",
            &input.title,
            &format!("{} {}", input.command_label, input.outcome.as_str()),
            &now,
        )?;
        let payload = json!({"test_run_id":id,"repository_id":input.repository_id,
            "baseline_id":input.baseline_id,"outcome":input.outcome.as_str(),
            "test_count":input.results.len()});
        let sequence = append_event_with_context(
            &tx,
            &self.manifest().project_id,
            command,
            Some(&id),
            "code_intelligence.test_run.recorded",
            &payload,
        )?;
        insert_development_timeline(
            &tx,
            &self.manifest().project_id,
            sequence,
            Some(&input.repository_id),
            Some(&id),
            "code_intelligence.test_run.recorded",
            &payload,
            &command.actor.id,
            &now,
        )?;
        record_command_with_context(
            &tx,
            command,
            &self.manifest().project_id,
            "RecordTestRun",
            Some(&id),
            None,
            &payload,
        )?;
        tx.commit()?;
        self.get_test_run(&id)
    }

    pub fn get_test_run(&self, id: &str) -> Result<TestRunRecord> {
        let entity = self.get_entity(id)?;
        if entity.entity_type != "test_run" {
            return Err(CoreError::Validation("entity is not a TestRun".into()));
        }
        let connection = self.connection()?;
        let raw = connection
            .query_row(
                "SELECT repository_id,baseline_id,outcome,command_label,exit_code,duration_ms,
                        observed_at,source_kind,details_json
                 FROM test_runs WHERE entity_id=?1 AND project_id=?2",
                params![id, self.manifest().project_id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, Option<i32>>(4)?,
                        row.get::<_, Option<i64>>(5)?,
                        row.get::<_, String>(6)?,
                        row.get::<_, String>(7)?,
                        row.get::<_, String>(8)?,
                    ))
                },
            )
            .optional()?
            .ok_or_else(|| CoreError::NotFound(id.into()))?;
        let mut statement = connection.prepare(
            "SELECT test_id,outcome,duration_ms,message FROM test_run_results
             WHERE test_run_id=?1 ORDER BY test_id",
        )?;
        let results = statement
            .query_map([id], |row| {
                Ok(TestRunResultRecord {
                    test_id: row.get(0)?,
                    outcome: row.get(1)?,
                    duration_ms: row.get::<_, Option<i64>>(2)?.map(|value| value as u64),
                    message: row.get(3)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(TestRunRecord {
            entity,
            repository_id: raw.0,
            baseline_id: raw.1,
            outcome: raw.2,
            command_label: raw.3,
            exit_code: raw.4,
            duration_ms: raw.5.map(|value: i64| value as u64),
            observed_at: raw.6,
            source_kind: raw.7,
            details: serde_json::from_str(&raw.8)?,
            results,
        })
    }

    pub fn link_test_verification(
        &self,
        command: &CommandContext,
        input: LinkTestVerificationInput,
    ) -> Result<String> {
        validate_command_context(command)?;
        reject_ai_actor(command)?;
        let mut connection = self.connection()?;
        let tx = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        require_development_enabled(&tx, &self.manifest().project_id)?;
        if let Some(Some(id)) = prior_result(
            &tx,
            &self.manifest().project_id,
            command,
            "LinkTestVerification",
        )? {
            tx.commit()?;
            return Ok(id);
        }
        require_typed_entity(&tx, &self.manifest().project_id, &input.test_id, "test")?;
        let repository_id: String = tx.query_row(
            "SELECT repository_id FROM tests WHERE entity_id=?1",
            [&input.test_id],
            |row| row.get(0),
        )?;
        let target_type: String = tx
            .query_row(
                "SELECT entity_type FROM entities WHERE id=?1 AND project_id=?2",
                params![input.target_entity_id, self.manifest().project_id],
                |row| row.get(0),
            )
            .optional()?
            .ok_or_else(|| CoreError::NotFound(input.target_entity_id.clone()))?;
        if !matches!(target_type.as_str(), "requirement" | "code_entity") {
            return Err(CoreError::Validation(
                "Test may verify only a Requirement or CodeEntity".into(),
            ));
        }
        if target_type == "code_entity" {
            let target_repository_id: String = tx.query_row(
                "SELECT repository_id FROM code_entities WHERE entity_id=?1",
                [&input.target_entity_id],
                |row| row.get(0),
            )?;
            if target_repository_id != repository_id {
                return Err(CoreError::Validation(
                    "Test cannot verify a CodeEntity from another Repository".into(),
                ));
            }
        }
        let id = insert_relationship_if_absent(
            &tx,
            &self.manifest().project_id,
            command,
            &input.test_id,
            "test",
            &input.target_entity_id,
            &target_type,
            "verifies",
            OriginKind::User,
            &[],
        )?;
        let payload = json!({"relationship_id":id,"test_id":input.test_id,
            "target_entity_id":input.target_entity_id,"target_entity_type":target_type});
        let sequence = append_event_with_context(
            &tx,
            &self.manifest().project_id,
            command,
            Some(&input.test_id),
            "code_intelligence.test_verification.linked",
            &payload,
        )?;
        let now = Utc::now().to_rfc3339();
        insert_development_timeline(
            &tx,
            &self.manifest().project_id,
            sequence,
            Some(&repository_id),
            Some(&input.test_id),
            "code_intelligence.test_verification.linked",
            &payload,
            &command.actor.id,
            &now,
        )?;
        record_command_with_context(
            &tx,
            command,
            &self.manifest().project_id,
            "LinkTestVerification",
            Some(&id),
            None,
            &payload,
        )?;
        tx.commit()?;
        Ok(id)
    }
}

fn prepare_analysis(
    connection: &Connection,
    root: &Path,
    baseline: &RepositoryBaselineRecord,
    limits: &AnalysisLimits,
) -> Result<PreparedAnalysis> {
    let dirty_paths = baseline
        .worktree_status
        .iter()
        .flat_map(|entry| [Some(entry.path.clone()), entry.original_path.clone()])
        .flatten()
        .collect::<BTreeSet<_>>();
    let tree_files = match baseline.head_oid.as_deref() {
        Some(head) => read_repository_tree_files(
            root,
            head,
            limits.max_files,
            limits.max_file_bytes,
            limits.max_total_bytes,
        )?,
        None => Vec::new(),
    };
    let mut files = Vec::with_capacity(tree_files.len() + baseline.worktree_status.len());
    let mut limitations = Vec::new();
    let mut cache_entries = Vec::new();
    let mut cache_hits = 0;
    let mut cache_misses = 0;
    let mut analyzed_files = 0;
    let tree_paths = tree_files
        .iter()
        .map(|file| file.path.clone())
        .collect::<HashSet<_>>();
    for file in tree_files {
        let dirty = dirty_paths.contains(&file.path);
        let prepared = prepare_file(connection, file, dirty)?;
        if prepared.cache_hit {
            cache_hits += 1;
        } else if prepared.analyzer_id != "file-fallback" {
            cache_misses += 1;
            cache_entries.push((
                prepared.content_sha256.clone(),
                prepared
                    .output
                    .language
                    .clone()
                    .unwrap_or_else(|| "plain".into()),
                serde_json::to_string(&prepared.output)?,
            ));
        }
        if prepared.status == "present" {
            analyzed_files += 1;
        }
        limitations.extend(prepared.limitations.iter().cloned());
        files.push(prepared);
    }
    for entry in &baseline.worktree_status {
        if tree_paths.contains(entry.path.as_str()) {
            continue;
        }
        if files.len() >= limits.max_files {
            return Err(CoreError::Conflict(format!(
                "repository baseline exceeds the configured {}-file CP5 analysis limit after working-tree fallbacks",
                limits.max_files
            )));
        }
        let content_sha256 = sha256_hex(
            format!(
                "unavailable-worktree\0{}\0{}",
                baseline.worktree_fingerprint, entry.path
            )
            .as_bytes(),
        );
        let limitation = PreparedLimitation {
            path: Some(entry.path.clone()),
            analyzer_id: "file-fallback".into(),
            code: "working_tree_content_not_captured".into(),
            severity: "warning".into(),
            message: "The path exists only in the CP4 working-tree coordinate; CP5 preserves a file-level fallback and does not claim structural content.".into(),
        };
        limitations.push(limitation.clone());
        files.push(PreparedFile {
            path: entry.path.clone(),
            mode: String::new(),
            object_type: "working_tree".into(),
            object_id: String::new(),
            content_sha256,
            output: empty_output(detect_language(&entry.path)),
            status: "fallback".into(),
            analyzer_id: "file-fallback".into(),
            analyzer_version: FALLBACK_ANALYZER_VERSION.into(),
            limitations: vec![limitation],
            cache_hit: false,
        });
    }
    if baseline.head_oid.is_none() {
        limitations.push(PreparedLimitation {
            path: None,
            analyzer_id: "file-fallback".into(),
            code: "unborn_repository".into(),
            severity: "info".into(),
            message: "The Repository has no commit tree; only CP4 working-tree path fallbacks are available.".into(),
        });
    }
    Ok(PreparedAnalysis {
        files,
        limitations,
        cache_entries,
        dirty_paths,
        cache_hits,
        cache_misses,
        analyzed_files,
    })
}

fn prepare_file(connection: &Connection, file: GitTreeFile, dirty: bool) -> Result<PreparedFile> {
    let language = detect_language(&file.path);
    let Some(bytes) = file.bytes.as_deref() else {
        let content_sha256 = sha256_hex(
            format!("unavailable-git-object\0{}\0{}", file.object_id, file.path).as_bytes(),
        );
        let code = if file.object_type == "blob" {
            "file_too_large"
        } else {
            "unsupported_git_object"
        };
        let limitation = PreparedLimitation {
            path: Some(file.path.clone()),
            analyzer_id: "file-fallback".into(),
            code: code.into(),
            severity: "warning".into(),
            message: "The path remains addressable at file level, but structural content was not analyzed.".into(),
        };
        return Ok(PreparedFile {
            path: file.path,
            mode: file.mode,
            object_type: file.object_type,
            object_id: file.object_id,
            content_sha256,
            output: empty_output(language),
            status: "fallback".into(),
            analyzer_id: "file-fallback".into(),
            analyzer_version: FALLBACK_ANALYZER_VERSION.into(),
            limitations: vec![limitation],
            cache_hit: false,
        });
    };
    let content_sha256 = sha256_hex(bytes);
    if file.mode == "120000" {
        let limitation = PreparedLimitation {
            path: Some(file.path.clone()),
            analyzer_id: "file-fallback".into(),
            code: "symlink_entry".into(),
            severity: "info".into(),
            message: "Git symlink targets are represented as hashed file-level data and are never parsed as source content.".into(),
        };
        return Ok(PreparedFile {
            path: file.path,
            mode: file.mode,
            object_type: file.object_type,
            object_id: file.object_id,
            content_sha256,
            output: empty_output(language),
            status: "fallback".into(),
            analyzer_id: "file-fallback".into(),
            analyzer_version: FALLBACK_ANALYZER_VERSION.into(),
            limitations: vec![limitation],
            cache_hit: false,
        });
    }
    if dirty {
        let limitation = PreparedLimitation {
            path: Some(file.path.clone()),
            analyzer_id: "file-fallback".into(),
            code: "working_tree_content_not_captured".into(),
            severity: "warning".into(),
            message: "The committed blob differs from or is affected by the CP4 working-tree state; structural output is withheld to avoid stale claims.".into(),
        };
        return Ok(PreparedFile {
            path: file.path,
            mode: file.mode,
            object_type: file.object_type,
            object_id: file.object_id,
            content_sha256,
            output: empty_output(language),
            status: "fallback".into(),
            analyzer_id: "file-fallback".into(),
            analyzer_version: FALLBACK_ANALYZER_VERSION.into(),
            limitations: vec![limitation],
            cache_hit: false,
        });
    }
    if bytes.contains(&0) {
        let limitation = PreparedLimitation {
            path: Some(file.path.clone()),
            analyzer_id: "file-fallback".into(),
            code: "binary_content".into(),
            severity: "info".into(),
            message: "Binary content is intentionally represented only as a hashed file entity."
                .into(),
        };
        return Ok(PreparedFile {
            path: file.path,
            mode: file.mode,
            object_type: file.object_type,
            object_id: file.object_id,
            content_sha256,
            output: empty_output(language),
            status: "fallback".into(),
            analyzer_id: "file-fallback".into(),
            analyzer_version: FALLBACK_ANALYZER_VERSION.into(),
            limitations: vec![limitation],
            cache_hit: false,
        });
    }
    let source = match std::str::from_utf8(bytes) {
        Ok(source) => source,
        Err(_) => {
            let limitation = PreparedLimitation {
                path: Some(file.path.clone()),
                analyzer_id: "file-fallback".into(),
                code: "non_utf8_content".into(),
                severity: "info".into(),
                message:
                    "Non-UTF-8 content is intentionally represented only as a hashed file entity."
                        .into(),
            };
            return Ok(PreparedFile {
                path: file.path,
                mode: file.mode,
                object_type: file.object_type,
                object_id: file.object_id,
                content_sha256,
                output: empty_output(language),
                status: "fallback".into(),
                analyzer_id: "file-fallback".into(),
                analyzer_version: FALLBACK_ANALYZER_VERSION.into(),
                limitations: vec![limitation],
                cache_hit: false,
            });
        }
    };
    let cache_language = language.clone().unwrap_or_else(|| "plain".into());
    if let Some(output) = read_cache(connection, &content_sha256, &cache_language)? {
        let limitations = if output.parser_had_error {
            vec![PreparedLimitation {
                path: Some(file.path.clone()),
                analyzer_id: "ast-grep-tree-sitter".into(),
                code: "parse_error".into(),
                severity: "warning".into(),
                message: "Tree-sitter recovered a partial syntax tree; extracted nodes are marked as partial analysis.".into(),
            }]
        } else {
            Vec::new()
        };
        return Ok(PreparedFile {
            path: file.path,
            mode: file.mode,
            object_type: file.object_type,
            object_id: file.object_id,
            content_sha256,
            output,
            status: "present".into(),
            analyzer_id: "ast-grep-tree-sitter".into(),
            analyzer_version: AST_GREP_VERSION.into(),
            limitations,
            cache_hit: true,
        });
    }
    let mut limitations = Vec::new();
    let output = analyze_text_file(&file.path, source, language.clone())?;
    if output.parser_had_error {
        limitations.push(PreparedLimitation {
            path: Some(file.path.clone()),
            analyzer_id: "ast-grep-tree-sitter".into(),
            code: "parse_error".into(),
            severity: "warning".into(),
            message: "Tree-sitter recovered a partial syntax tree; extracted nodes are marked as partial analysis.".into(),
        });
    }
    if language.is_none() && output.configuration.is_none() && output.dependencies.is_empty() {
        limitations.push(PreparedLimitation {
            path: Some(file.path.clone()),
            analyzer_id: "file-fallback".into(),
            code: "unsupported_language".into(),
            severity: "info".into(),
            message: "No pinned structural grammar is enabled for this path; file identity remains available.".into(),
        });
    }
    let structured =
        language.is_some() || output.configuration.is_some() || !output.dependencies.is_empty();
    Ok(PreparedFile {
        path: file.path,
        mode: file.mode,
        object_type: file.object_type,
        object_id: file.object_id,
        content_sha256,
        output,
        status: if structured { "present" } else { "fallback" }.into(),
        analyzer_id: if structured {
            "ast-grep-tree-sitter"
        } else {
            "file-fallback"
        }
        .into(),
        analyzer_version: if structured {
            AST_GREP_VERSION
        } else {
            FALLBACK_ANALYZER_VERSION
        }
        .into(),
        limitations,
        cache_hit: false,
    })
}

fn analyze_text_file(
    path: &str,
    source: &str,
    language: Option<String>,
) -> Result<ParsedFileOutput> {
    let mut output = empty_output(language.clone());
    if let Some(language_name) = language.as_deref() {
        let lang = support_language(language_name).ok_or_else(|| {
            CoreError::Validation(format!(
                "unsupported configured analyzer language: {language_name}"
            ))
        })?;
        let root = lang.ast_grep(source);
        let root_node = root.root();
        output.parser_had_error = root_node
            .dfs()
            .any(|node| node.is_error() || node.is_missing());
        for node in root_node.dfs() {
            let kind = node.kind();
            let Some(symbol_kind) = symbol_kind(language_name, &kind) else {
                continue;
            };
            let Some(name_node) = node.field("name") else {
                continue;
            };
            let name = bounded_fragment(&name_node.text(), 300);
            if name.is_empty() {
                continue;
            }
            let qualified_name = qualified_name(&node, &name);
            let text = node.text();
            let start = node.start_pos();
            let end = node.end_pos();
            let symbol = ParsedSymbol {
                kind: symbol_kind.into(),
                name: name.clone(),
                qualified_name: qualified_name.clone(),
                start_line: start.line() as u64 + 1,
                start_column: start.byte_point().1 as u64 + 1,
                end_line: end.line() as u64 + 1,
                end_column: end.byte_point().1 as u64 + 1,
                visibility: detect_visibility(language_name, &name, &text),
                content_sha256: sha256_hex(text.as_bytes()),
                signature_sha256: sha256_hex(normalized_signature(&text).as_bytes()),
            };
            if is_structural_test(language_name, &node, source, &name) {
                output.tests.push(ParsedTest {
                    name: name.clone(),
                    qualified_name: qualified_name.clone(),
                    framework: test_framework(language_name, source, node.range().start).into(),
                    test_kind: "test".into(),
                    start_line: symbol.start_line,
                    end_line: symbol.end_line,
                    content_sha256: symbol.content_sha256.clone(),
                    symbol_qualified_name: Some(qualified_name.clone()),
                });
            }
            output.symbols.push(symbol);
        }
        if matches!(language_name, "javascript" | "typescript" | "tsx") {
            collect_ast_grep_js_tests(&root_node, &mut output.tests);
        }
    }
    output.dependencies = analyze_dependencies(path, source)?;
    output.configuration = analyze_configuration(path, source);
    deduplicate_parsed(&mut output);
    Ok(output)
}

fn collect_ast_grep_js_tests<D: ast_grep_core::Doc>(
    root: &ast_grep_core::Node<'_, D>,
    tests: &mut Vec<ParsedTest>,
) where
    D::Lang: ast_grep_core::Language,
{
    for (callee, kind) in [("test", "test"), ("it", "test"), ("describe", "suite")] {
        let pattern = format!("{callee}($NAME, $$$ARGS)");
        for matched in root.find_all(pattern.as_str()) {
            let text = matched.text();
            let name = first_quoted_argument(&text).unwrap_or_else(|| bounded_fragment(&text, 200));
            let start = matched.start_pos();
            let end = matched.end_pos();
            tests.push(ParsedTest {
                name: name.clone(),
                qualified_name: format!("{callee}:{name}"),
                framework: "javascript-test-api".into(),
                test_kind: kind.into(),
                start_line: start.line() as u64 + 1,
                end_line: end.line() as u64 + 1,
                content_sha256: sha256_hex(text.as_bytes()),
                symbol_qualified_name: None,
            });
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn persist_analysis(
    store: &ContinuityStore,
    tx: Transaction<'_>,
    command: &CommandContext,
    repository_id: &str,
    baseline: &RepositoryBaselineRecord,
    source_fingerprint: &str,
    limits: &AnalysisLimits,
    started_at: &str,
    completed_at: &str,
    duration_ms: u64,
    prepared: PreparedAnalysis,
) -> Result<AnalysisRunRecord> {
    let project_id = &store.manifest().project_id;
    let apply_current_projection =
        is_current_repository_baseline(&tx, repository_id, &baseline.entity.id)?;
    let run_id = new_id();
    let projected_count = estimate_entity_count(&prepared);
    if projected_count > limits.max_entities {
        return Err(CoreError::Conflict(format!(
            "analysis produced {projected_count} addressable records, above configured max_entities {}",
            limits.max_entities
        )));
    }
    let completeness = if prepared.limitations.is_empty() {
        "complete"
    } else {
        "partial"
    };
    let (expected_code_count, expected_test_count) = expected_unique_counts(&prepared);
    let metadata_json = bounded_json(
        &json!({"analyzer_bundle_version":ANALYZER_BUNDLE_VERSION,
            "output_schema_version":OUTPUT_SCHEMA_VERSION}),
        MAX_JSON_BYTES,
        "AnalysisRun metadata",
    )?;
    insert_common_entity(
        &tx,
        project_id,
        command,
        &run_id,
        "analysis_run",
        &format!("Code analysis {}", short_id(&baseline.entity.id)),
        completeness,
        OriginKind::Deterministic,
        &metadata_json,
        &json!({"repository_id":repository_id,"baseline_id":baseline.entity.id,
            "source_fingerprint":source_fingerprint}),
        completed_at,
    )?;
    let limits_json = bounded_json(
        &serde_json::to_value(limits)?,
        MAX_JSON_BYTES,
        "analysis limits",
    )?;
    tx.execute(
        "INSERT INTO analysis_runs(entity_id,project_id,repository_id,baseline_id,source_fingerprint,
            analyzer_bundle_version,output_schema_version,completeness,file_count,analyzed_file_count,
            code_entity_count,test_count,limitation_count,cache_hit_count,cache_miss_count,limits_json,
            started_at,completed_at,duration_ms)
         VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19)",
        params![run_id,project_id,repository_id,baseline.entity.id,source_fingerprint,
            ANALYZER_BUNDLE_VERSION,OUTPUT_SCHEMA_VERSION,completeness,prepared.files.len() as i64,
            prepared.analyzed_files as i64,expected_code_count as i64,expected_test_count as i64,
            prepared.limitations.len() as i64,prepared.cache_hits as i64,
            prepared.cache_misses as i64,limits_json,started_at,completed_at,duration_ms as i64],
    )?;
    for (hash, language, output_json) in &prepared.cache_entries {
        tx.execute(
            "INSERT INTO analyzer_cache(content_sha256,language,analyzer_id,analyzer_version,
                output_schema_version,output_json,byte_size,created_at,last_accessed_at)
             VALUES(?1,?2,'code-intelligence-file',?3,?4,?5,?6,?7,?7)
             ON CONFLICT(content_sha256,language,analyzer_id,analyzer_version,output_schema_version)
             DO UPDATE SET last_accessed_at=excluded.last_accessed_at",
            params![
                hash,
                language,
                FILE_ANALYSIS_CACHE_VERSION,
                OUTPUT_SCHEMA_VERSION,
                output_json,
                output_json.len() as i64,
                completed_at
            ],
        )?;
    }
    enforce_analyzer_cache_budget(&tx)?;
    let mut code_ids = HashSet::new();
    let mut test_ids = HashSet::new();
    let mut file_ids = HashMap::new();
    let current_file_hashes = prepared
        .files
        .iter()
        .map(|file| (file.path.clone(), file.content_sha256.clone()))
        .collect::<HashMap<_, _>>();
    let rename_destinations =
        rename_destinations_first_observed_at_baseline(&tx, repository_id, &baseline.entity.id)?;
    let mut ordered_files = prepared.files.iter().collect::<Vec<_>>();
    ordered_files.sort_by(|left, right| {
        let left_rank = !rename_destinations.contains(&left.path);
        let right_rank = !rename_destinations.contains(&right.path);
        left_rank.cmp(&right_rank).then(left.path.cmp(&right.path))
    });
    for file in ordered_files {
        let file_id = upsert_code_entity(
            &tx,
            project_id,
            command,
            repository_id,
            &baseline.entity.id,
            "file",
            &format!("file:{}", file.path),
            &file.path,
            file.output.language.as_deref(),
            Some((&file.path, "path")),
            Some(&file.content_sha256),
            Some(&current_file_hashes),
            apply_current_projection,
            completed_at,
        )?;
        file_ids.insert(file.path.clone(), file_id.clone());
        code_ids.insert(file_id.clone());
        insert_code_observation(
            &tx,
            project_id,
            &run_id,
            &baseline.entity.id,
            &file_id,
            &file.path,
            None,
            &file.content_sha256,
            &sha256_hex(format!("file\0{}", file.path).as_bytes()),
            None,
            None,
            None,
            None,
            None,
            &file.status,
            &file.analyzer_id,
            &file.analyzer_version,
            &json!({"git_mode":file.mode,"git_object_type":file.object_type,
                "git_object_id":file.object_id,"dirty_fallback":prepared.dirty_paths.contains(&file.path)}),
            completed_at,
        )?;
        insert_relationship_if_absent(
            &tx,
            project_id,
            command,
            repository_id,
            "repository",
            &file_id,
            "code_entity",
            "defines",
            OriginKind::Deterministic,
            &[baseline.entity.id.clone(), run_id.clone()],
        )?;
        for symbol in &file.output.symbols {
            let stable_key = format!("symbol:{file_id}:{}:{}", symbol.kind, symbol.qualified_name);
            let symbol_id = upsert_code_entity(
                &tx,
                project_id,
                command,
                repository_id,
                &baseline.entity.id,
                "symbol",
                &stable_key,
                &symbol.qualified_name,
                file.output.language.as_deref(),
                None,
                None,
                None,
                apply_current_projection,
                completed_at,
            )?;
            code_ids.insert(symbol_id.clone());
            insert_code_observation(
                &tx,
                project_id,
                &run_id,
                &baseline.entity.id,
                &symbol_id,
                &file.path,
                Some(&symbol.qualified_name),
                &symbol.content_sha256,
                &symbol.signature_sha256,
                Some(symbol.start_line),
                Some(symbol.start_column),
                Some(symbol.end_line),
                Some(symbol.end_column),
                symbol.visibility.as_deref(),
                if file.output.parser_had_error {
                    "parse_error"
                } else {
                    "present"
                },
                "ast-grep-tree-sitter",
                AST_GREP_VERSION,
                &json!({"symbol_kind":symbol.kind,"name":symbol.name,"parent_file_id":file_id}),
                completed_at,
            )?;
            insert_relationship_if_absent(
                &tx,
                project_id,
                command,
                repository_id,
                "repository",
                &symbol_id,
                "code_entity",
                "defines",
                OriginKind::Deterministic,
                &[baseline.entity.id.clone(), run_id.clone()],
            )?;
        }
        for dependency in &file.output.dependencies {
            let stable_key = format!(
                "dependency:{}:{}:{}",
                dependency.ecosystem, dependency.scope, dependency.name
            );
            let title = format!("{} ({})", dependency.name, dependency.scope);
            let dependency_id = upsert_code_entity(
                &tx,
                project_id,
                command,
                repository_id,
                &baseline.entity.id,
                "dependency",
                &stable_key,
                &title,
                None,
                None,
                None,
                None,
                apply_current_projection,
                completed_at,
            )?;
            code_ids.insert(dependency_id.clone());
            let declaration_hash = sha256_hex(
                format!(
                    "{}\0{}\0{}",
                    dependency.name, dependency.scope, dependency.requirement
                )
                .as_bytes(),
            );
            insert_code_observation(
                &tx,
                project_id,
                &run_id,
                &baseline.entity.id,
                &dependency_id,
                &file.path,
                Some(&dependency.name),
                &declaration_hash,
                &declaration_hash,
                None,
                None,
                None,
                None,
                None,
                "present",
                "manifest-dependency-config",
                MANIFEST_ANALYZER_VERSION,
                &json!({"ecosystem":dependency.ecosystem,"scope":dependency.scope,
                    "requirement":dependency.requirement}),
                completed_at,
            )?;
            insert_relationship_if_absent(
                &tx,
                project_id,
                command,
                repository_id,
                "repository",
                &dependency_id,
                "code_entity",
                "defines",
                OriginKind::Deterministic,
                &[baseline.entity.id.clone(), run_id.clone()],
            )?;
        }
        if let Some(configuration) = &file.output.configuration {
            let stable_key = format!("configuration:{file_id}");
            let configuration_id = upsert_code_entity(
                &tx,
                project_id,
                command,
                repository_id,
                &baseline.entity.id,
                "configuration",
                &stable_key,
                &file.path,
                None,
                None,
                None,
                None,
                apply_current_projection,
                completed_at,
            )?;
            code_ids.insert(configuration_id.clone());
            insert_code_observation(
                &tx,
                project_id,
                &run_id,
                &baseline.entity.id,
                &configuration_id,
                &file.path,
                Some(&file.path),
                &file.content_sha256,
                &file.content_sha256,
                None,
                None,
                None,
                None,
                None,
                "present",
                "manifest-dependency-config",
                MANIFEST_ANALYZER_VERSION,
                &serde_json::to_value(configuration)?,
                completed_at,
            )?;
            insert_relationship_if_absent(
                &tx,
                project_id,
                command,
                repository_id,
                "repository",
                &configuration_id,
                "code_entity",
                "defines",
                OriginKind::Deterministic,
                &[baseline.entity.id.clone(), run_id.clone()],
            )?;
        }
    }
    for file in &prepared.files {
        let Some(file_id) = file_ids.get(&file.path) else {
            continue;
        };
        for test in &file.output.tests {
            let stable_key = format!("test:{file_id}:{}:{}", test.framework, test.qualified_name);
            let test_id = upsert_test(
                &tx,
                project_id,
                command,
                repository_id,
                &baseline.entity.id,
                &stable_key,
                &test.name,
                &test.framework,
                &test.test_kind,
                apply_current_projection,
                completed_at,
            )?;
            test_ids.insert(test_id.clone());
            let code_entity_id = test
                .symbol_qualified_name
                .as_deref()
                .and_then(|name| {
                    find_symbol_id(&tx, repository_id, file_id, name)
                        .ok()
                        .flatten()
                })
                .unwrap_or_else(|| file_id.clone());
            tx.execute(
                "INSERT INTO test_observations(id,project_id,analysis_run_id,baseline_id,test_id,
                    code_entity_id,source_path,qualified_name,content_sha256,start_line,end_line,
                    analyzer_id,analyzer_version,details_json,observed_at)
                 VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,'ast-grep-test-patterns',?12,?13,?14)",
                params![
                    new_id(),
                    project_id,
                    run_id,
                    baseline.entity.id,
                    test_id,
                    code_entity_id,
                    file.path,
                    test.qualified_name,
                    test.content_sha256,
                    test.start_line as i64,
                    test.end_line as i64,
                    AST_GREP_VERSION,
                    bounded_json(
                        &json!({"framework":test.framework,
                        "test_kind":test.test_kind}),
                        MAX_JSON_BYTES,
                        "Test observation details"
                    )?,
                    completed_at
                ],
            )?;
            insert_relationship_if_absent(
                &tx,
                project_id,
                command,
                repository_id,
                "repository",
                &test_id,
                "test",
                "defines",
                OriginKind::Deterministic,
                &[baseline.entity.id.clone(), run_id.clone()],
            )?;
        }
    }
    link_change_sets_to_files(
        &tx,
        project_id,
        command,
        repository_id,
        &baseline.entity.id,
        &file_ids,
        &run_id,
    )?;
    for limitation in &prepared.limitations {
        tx.execute(
            "INSERT INTO analysis_limitations(id,project_id,analysis_run_id,source_path,analyzer_id,
                code,severity,message) VALUES(?1,?2,?3,?4,?5,?6,?7,?8)",
            params![new_id(),project_id,run_id,limitation.path,limitation.analyzer_id,
                limitation.code,limitation.severity,limitation.message],
        )?;
    }
    let structural_outputs = prepared
        .files
        .iter()
        .map(|file| file.output.symbols.len())
        .sum::<usize>();
    let test_outputs = prepared
        .files
        .iter()
        .map(|file| file.output.tests.len())
        .sum::<usize>();
    let manifest_outputs = prepared
        .files
        .iter()
        .map(|file| {
            file.output.dependencies.len() + usize::from(file.output.configuration.is_some())
        })
        .sum::<usize>();
    let executions = [
        ("ast-grep-tree-sitter", AST_GREP_VERSION, structural_outputs),
        ("ast-grep-test-patterns", AST_GREP_VERSION, test_outputs),
        (
            "manifest-dependency-config",
            MANIFEST_ANALYZER_VERSION,
            manifest_outputs,
        ),
        (
            "file-fallback",
            FALLBACK_ANALYZER_VERSION,
            prepared.files.len(),
        ),
    ];
    for (ordinal, (analyzer_id, version, outputs)) in executions.into_iter().enumerate() {
        let related_limitations = prepared
            .limitations
            .iter()
            .filter(|item| item.analyzer_id == analyzer_id)
            .count();
        tx.execute(
            "INSERT INTO analyzer_executions(analysis_run_id,ordinal,analyzer_id,analyzer_version,
                contract_version,output_schema_version,status,files_seen,outputs_created,cache_hits,
                cache_misses,duration_ms,diagnostic_code)
             VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13)",
            params![
                run_id,
                ordinal as i64,
                analyzer_id,
                version,
                ANALYZER_CONTRACT_VERSION,
                OUTPUT_SCHEMA_VERSION,
                if related_limitations == 0 {
                    "succeeded"
                } else {
                    "degraded"
                },
                prepared.files.len() as i64,
                outputs as i64,
                prepared.cache_hits as i64,
                prepared.cache_misses as i64,
                duration_ms as i64,
                (related_limitations > 0).then_some("limitations_recorded")
            ],
        )?;
    }
    if code_ids.len() != expected_code_count || test_ids.len() != expected_test_count {
        return Err(CoreError::Conflict(
            "deterministic analyzer identity count changed during persistence".into(),
        ));
    }
    if apply_current_projection {
        let _ = mark_unobserved_code_intelligence(
            &tx,
            project_id,
            command,
            repository_id,
            &baseline.entity.id,
            &code_ids,
            &test_ids,
            completed_at,
        )?;
    }
    upsert_development_search(
        &tx,
        project_id,
        &run_id,
        "analysis_run",
        &format!("Code analysis {}", short_id(&baseline.entity.id)),
        &format!(
            "{completeness} {} files {} code entities {} tests {} limitations",
            prepared.files.len(),
            code_ids.len(),
            test_ids.len(),
            prepared.limitations.len()
        ),
        completed_at,
    )?;
    let payload = json!({"analysis_run_id":run_id,"repository_id":repository_id,
        "baseline_id":baseline.entity.id,"completeness":completeness,
        "file_count":prepared.files.len(),"code_entity_count":code_ids.len(),
        "test_count":test_ids.len(),"limitation_count":prepared.limitations.len(),
        "analyzer_bundle_version":ANALYZER_BUNDLE_VERSION,
        "output_schema_version":OUTPUT_SCHEMA_VERSION});
    let sequence = append_event_with_context(
        &tx,
        project_id,
        command,
        Some(&run_id),
        "code_intelligence.analysis.completed",
        &payload,
    )?;
    insert_development_timeline(
        &tx,
        project_id,
        sequence,
        Some(repository_id),
        Some(&run_id),
        "code_intelligence.analysis.completed",
        &payload,
        &command.actor.id,
        completed_at,
    )?;
    record_command_with_context(
        &tx,
        command,
        project_id,
        "AnalyzeRepositoryBaseline",
        Some(&run_id),
        None,
        &payload,
    )?;
    tx.commit()?;
    store.get_analysis_run(&run_id)
}

#[allow(clippy::too_many_arguments)]
fn upsert_code_entity(
    tx: &Transaction<'_>,
    project_id: &str,
    command: &CommandContext,
    repository_id: &str,
    baseline_id: &str,
    kind: &str,
    stable_key: &str,
    title: &str,
    language: Option<&str>,
    alias: Option<(&str, &str)>,
    content_hash_for_rename: Option<&str>,
    current_file_hashes: Option<&HashMap<String, String>>,
    apply_current_projection: bool,
    now: &str,
) -> Result<String> {
    let mut renamed_from: Option<String> = None;
    let mut id: Option<String> = None;
    if kind == "file"
        && let (Some((new_path, "path")), Some(hash)) = (alias, content_hash_for_rename)
    {
        let rename: Option<(String, String, String)> = tx
            .query_row(
                "SELECT cea.code_entity_id,fc.old_path,fc.change_kind
                 FROM git_commit_file_changes fc
                 JOIN git_commit_observations co ON co.entity_id=fc.commit_entity_id
                 JOIN code_entity_aliases cea ON cea.repository_id=co.repository_id
                    AND cea.alias_kind='path' AND cea.alias_value=fc.old_path
                    AND cea.retired_at_baseline_id IS NULL
                 JOIN code_entity_observations ceo ON ceo.code_entity_id=cea.code_entity_id
                    AND ceo.content_sha256=?3
                 WHERE co.repository_id=?1 AND fc.change_kind IN ('renamed','copied')
                   AND fc.new_path=?2
                 ORDER BY co.committed_at DESC,ceo.observed_at DESC LIMIT 1",
                params![repository_id, new_path, hash],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .optional()?;
        if let Some((rename_id, old_path, change_kind)) = rename {
            let old_path_reused_with_other_content = current_file_hashes
                .and_then(|files| files.get(&old_path))
                .is_some_and(|current_hash| current_hash != hash);
            if change_kind == "renamed" || old_path_reused_with_other_content {
                id = Some(rename_id);
                renamed_from = Some(old_path);
            }
        }
    }
    if id.is_none()
        && apply_current_projection
        && let Some((alias_value, alias_kind)) = alias
    {
        id = tx
            .query_row(
                "SELECT code_entity_id FROM code_entity_aliases
                 WHERE repository_id=?1 AND alias_kind=?2 AND alias_value=?3
                   AND retired_at_baseline_id IS NULL",
                params![repository_id, alias_kind, alias_value],
                |row| row.get(0),
            )
            .optional()?;
    }
    if id.is_none() && kind != "file" {
        id = tx
            .query_row(
                "SELECT entity_id FROM code_entities
                 WHERE repository_id=?1 AND entity_kind=?2 AND stable_key=?3",
                params![repository_id, kind, stable_key],
                |row| row.get(0),
            )
            .optional()?;
    }
    if id.is_none()
        && kind == "file"
        && let (Some((alias_value, "path")), Some(hash)) = (alias, content_hash_for_rename)
    {
        // Content corroboration prevents a historical analysis or restored
        // path from inheriting an unrelated file's identity.
        id = tx
            .query_row(
                "SELECT cea.code_entity_id FROM code_entity_aliases cea
                 JOIN code_entity_observations ceo ON ceo.code_entity_id=cea.code_entity_id
                 WHERE cea.repository_id=?1 AND cea.alias_kind='path' AND cea.alias_value=?2
                   AND ceo.content_sha256=?3
                 ORDER BY ceo.observed_at DESC LIMIT 1",
                params![repository_id, alias_value, hash],
                |row| row.get(0),
            )
            .optional()?;
    }
    if let Some(id) = id {
        if apply_current_projection {
            tx.execute(
                "UPDATE entities SET title=?3,status='active',version=version+1,updated_at=?4,updated_by=?5,
                    data_json=?6 WHERE id=?1 AND project_id=?2 AND entity_type='code_entity'",
                params![
                    id,
                    project_id,
                    title,
                    now,
                    command.actor.id,
                    bounded_json(
                        &json!({"stable_key":stable_key,"presence":"present",
                            "last_observed_baseline_id":baseline_id}),
                        MAX_JSON_BYTES,
                        "CodeEntity current data"
                    )?
                ],
            )?;
            if let Some(old_path) = renamed_from {
                tx.execute(
                    "UPDATE code_entity_aliases SET last_seen_baseline_id=?4,
                        retired_at_baseline_id=?4
                     WHERE repository_id=?1 AND alias_kind='path' AND alias_value=?2
                       AND code_entity_id=?3 AND retired_at_baseline_id IS NULL",
                    params![repository_id, old_path, id, baseline_id],
                )?;
            }
            if let Some((alias_value, alias_kind)) = alias {
                activate_code_entity_alias(
                    tx,
                    repository_id,
                    alias_kind,
                    alias_value,
                    &id,
                    baseline_id,
                )?;
            }
            upsert_development_search(tx, project_id, &id, "code_entity", title, stable_key, now)?;
        }
        return Ok(id);
    }
    let id = new_id();
    let effective_stable_key = if kind == "file" {
        format!("file:{id}")
    } else {
        stable_key.to_owned()
    };
    insert_common_entity(
        tx,
        project_id,
        command,
        &id,
        "code_entity",
        title,
        if apply_current_projection {
            "active"
        } else {
            "unavailable"
        },
        OriginKind::Deterministic,
        "{}",
        &if apply_current_projection {
            json!({"stable_key":effective_stable_key,"presence":"present",
                "last_observed_baseline_id":baseline_id})
        } else {
            json!({"stable_key":effective_stable_key,"presence":"historical",
                "historical_baseline_id":baseline_id})
        },
        now,
    )?;
    tx.execute(
        "INSERT INTO code_entities(entity_id,project_id,repository_id,entity_kind,stable_key,language,
            first_seen_baseline_id) VALUES(?1,?2,?3,?4,?5,?6,?7)",
        params![id, project_id, repository_id, kind, effective_stable_key, language, baseline_id],
    )?;
    if apply_current_projection && let Some((alias_value, alias_kind)) = alias {
        activate_code_entity_alias(tx, repository_id, alias_kind, alias_value, &id, baseline_id)?;
    }
    upsert_development_search(
        tx,
        project_id,
        &id,
        "code_entity",
        title,
        &effective_stable_key,
        now,
    )?;
    Ok(id)
}

fn activate_code_entity_alias(
    tx: &Transaction<'_>,
    repository_id: &str,
    alias_kind: &str,
    alias_value: &str,
    code_entity_id: &str,
    baseline_id: &str,
) -> Result<()> {
    tx.execute(
        "UPDATE code_entity_aliases SET last_seen_baseline_id=?4,retired_at_baseline_id=?4
         WHERE repository_id=?1 AND alias_kind=?2 AND alias_value=?3
           AND code_entity_id<>?5 AND retired_at_baseline_id IS NULL",
        params![
            repository_id,
            alias_kind,
            alias_value,
            baseline_id,
            code_entity_id
        ],
    )?;
    tx.execute(
        "INSERT INTO code_entity_aliases(repository_id,alias_kind,alias_value,code_entity_id,
            first_seen_baseline_id,last_seen_baseline_id,retired_at_baseline_id)
         VALUES(?1,?2,?3,?4,?5,?5,NULL)
         ON CONFLICT(repository_id,alias_kind,alias_value,code_entity_id)
         DO UPDATE SET last_seen_baseline_id=excluded.last_seen_baseline_id,
                       retired_at_baseline_id=NULL",
        params![
            repository_id,
            alias_kind,
            alias_value,
            code_entity_id,
            baseline_id
        ],
    )?;
    Ok(())
}

fn rename_destinations_first_observed_at_baseline(
    connection: &Connection,
    repository_id: &str,
    baseline_id: &str,
) -> Result<HashSet<String>> {
    let mut statement = connection.prepare(
        "SELECT DISTINCT fc.new_path FROM git_commit_file_changes fc
         JOIN git_commit_observations co ON co.entity_id=fc.commit_entity_id
         WHERE co.repository_id=?1 AND co.first_observed_baseline_id=?2
           AND fc.change_kind IN ('renamed','copied')",
    )?;
    statement
        .query_map(params![repository_id, baseline_id], |row| row.get(0))?
        .collect::<std::result::Result<HashSet<_>, _>>()
        .map_err(Into::into)
}

#[allow(clippy::too_many_arguments)]
fn insert_code_observation(
    tx: &Transaction<'_>,
    project_id: &str,
    run_id: &str,
    baseline_id: &str,
    code_entity_id: &str,
    path: &str,
    qualified_name: Option<&str>,
    content_sha256: &str,
    signature_sha256: &str,
    start_line: Option<u64>,
    start_column: Option<u64>,
    end_line: Option<u64>,
    end_column: Option<u64>,
    visibility: Option<&str>,
    status: &str,
    analyzer_id: &str,
    analyzer_version: &str,
    details: &Value,
    observed_at: &str,
) -> Result<()> {
    tx.execute(
        "INSERT INTO code_entity_observations(id,project_id,analysis_run_id,baseline_id,code_entity_id,
            source_path,qualified_name,content_sha256,signature_sha256,start_line,start_column,end_line,
            end_column,visibility,observation_status,analyzer_id,analyzer_version,output_schema_version,
            details_json,observed_at)
         VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19,?20)",
        params![new_id(),project_id,run_id,baseline_id,code_entity_id,path,qualified_name,
            content_sha256,signature_sha256,start_line.map(|v|v as i64),
            start_column.map(|v|v as i64),end_line.map(|v|v as i64),
            end_column.map(|v|v as i64),visibility,status,analyzer_id,analyzer_version,
            OUTPUT_SCHEMA_VERSION,bounded_json(details,MAX_JSON_BYTES,
                "CodeEntity observation details")?,observed_at],
    )?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn upsert_test(
    tx: &Transaction<'_>,
    project_id: &str,
    command: &CommandContext,
    repository_id: &str,
    baseline_id: &str,
    stable_key: &str,
    title: &str,
    framework: &str,
    test_kind: &str,
    apply_current_projection: bool,
    now: &str,
) -> Result<String> {
    if let Some(id) = tx
        .query_row(
            "SELECT entity_id FROM tests WHERE repository_id=?1 AND stable_key=?2",
            params![repository_id, stable_key],
            |row| row.get::<_, String>(0),
        )
        .optional()?
    {
        if apply_current_projection {
            tx.execute(
            "UPDATE entities SET title=?3,status='active',version=version+1,updated_at=?4,updated_by=?5,
                data_json=?6 WHERE id=?1 AND project_id=?2 AND entity_type='test'",
            params![
                id,
                project_id,
                title,
                now,
                command.actor.id,
                bounded_json(
                    &json!({"stable_key":stable_key,"presence":"present",
                        "last_observed_baseline_id":baseline_id}),
                    MAX_JSON_BYTES,
                    "Test current data"
                )?
            ],
            )?;
            upsert_development_search(
                tx,
                project_id,
                &id,
                "test",
                title,
                &format!("{framework} {test_kind}"),
                now,
            )?;
        }
        return Ok(id);
    }
    let id = new_id();
    insert_common_entity(
        tx,
        project_id,
        command,
        &id,
        "test",
        title,
        if apply_current_projection {
            "active"
        } else {
            "unavailable"
        },
        OriginKind::Deterministic,
        "{}",
        &if apply_current_projection {
            json!({"stable_key":stable_key,"presence":"present",
                "last_observed_baseline_id":baseline_id})
        } else {
            json!({"stable_key":stable_key,"presence":"historical",
                "historical_baseline_id":baseline_id})
        },
        now,
    )?;
    tx.execute(
        "INSERT INTO tests(entity_id,project_id,repository_id,stable_key,framework,test_kind,
            first_seen_baseline_id) VALUES(?1,?2,?3,?4,?5,?6,?7)",
        params![
            id,
            project_id,
            repository_id,
            stable_key,
            framework,
            test_kind,
            baseline_id
        ],
    )?;
    upsert_development_search(
        tx,
        project_id,
        &id,
        "test",
        title,
        &format!("{framework} {test_kind}"),
        now,
    )?;
    Ok(id)
}

fn reapply_analysis_projection(
    store: &ContinuityStore,
    tx: Transaction<'_>,
    command: &CommandContext,
    run_id: &str,
    repository_id: &str,
    baseline_id: &str,
) -> Result<AnalysisRunRecord> {
    require_development_enabled(&tx, &store.manifest().project_id)?;
    let apply_current_projection = is_current_repository_baseline(&tx, repository_id, baseline_id)?;
    let code_ids = tx
        .prepare(
            "SELECT DISTINCT code_entity_id FROM code_entity_observations
             WHERE analysis_run_id=?1 ORDER BY code_entity_id",
        )?
        .query_map([run_id], |row| row.get::<_, String>(0))?
        .collect::<std::result::Result<HashSet<_>, _>>()?;
    let test_ids = tx
        .prepare(
            "SELECT DISTINCT test_id FROM test_observations
             WHERE analysis_run_id=?1 ORDER BY test_id",
        )?
        .query_map([run_id], |row| row.get::<_, String>(0))?
        .collect::<std::result::Result<HashSet<_>, _>>()?;
    let now = Utc::now().to_rfc3339();
    let mut changed = 0;
    if apply_current_projection {
        changed = reactivate_observed_entities(
            &tx,
            &store.manifest().project_id,
            command,
            baseline_id,
            &code_ids,
            &test_ids,
            &now,
        )?;
        changed += mark_unobserved_code_intelligence(
            &tx,
            &store.manifest().project_id,
            command,
            repository_id,
            baseline_id,
            &code_ids,
            &test_ids,
            &now,
        )?;
    }
    let payload = json!({"reused":true,"analysis_run_id":run_id,
        "repository_id":repository_id,"baseline_id":baseline_id,
        "applied_current_projection":apply_current_projection,
        "projection_changes":changed});
    if changed > 0 {
        let sequence = append_event_with_context(
            &tx,
            &store.manifest().project_id,
            command,
            Some(run_id),
            "code_intelligence.analysis.reapplied",
            &payload,
        )?;
        insert_development_timeline(
            &tx,
            &store.manifest().project_id,
            sequence,
            Some(repository_id),
            Some(run_id),
            "code_intelligence.analysis.reapplied",
            &payload,
            &command.actor.id,
            &now,
        )?;
    }
    record_command_with_context(
        &tx,
        command,
        &store.manifest().project_id,
        "AnalyzeRepositoryBaseline",
        Some(run_id),
        None,
        &payload,
    )?;
    tx.commit()?;
    store.get_analysis_run(run_id)
}

#[allow(clippy::too_many_arguments)]
fn reactivate_observed_entities(
    tx: &Transaction<'_>,
    project_id: &str,
    command: &CommandContext,
    baseline_id: &str,
    code_ids: &HashSet<String>,
    test_ids: &HashSet<String>,
    now: &str,
) -> Result<usize> {
    let mut changed = 0_usize;
    for (entity_type, ids) in [("code_entity", code_ids), ("test", test_ids)] {
        for id in ids {
            let (status, raw_data): (String, String) = tx.query_row(
                "SELECT status,data_json FROM entities
                 WHERE id=?1 AND project_id=?2 AND entity_type=?3",
                params![id, project_id, entity_type],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )?;
            let mut data: Value = serde_json::from_str(&raw_data)?;
            let object = data.as_object_mut().ok_or_else(|| {
                CoreError::Conflict(format!(
                    "{entity_type} {id} has non-object current data; run integrity diagnostics"
                ))
            })?;
            let already_current = status == "active"
                && object.get("presence").and_then(Value::as_str) == Some("present")
                && object
                    .get("last_observed_baseline_id")
                    .and_then(Value::as_str)
                    == Some(baseline_id);
            if already_current {
                continue;
            }
            object.insert("presence".into(), Value::String("present".into()));
            object.insert(
                "last_observed_baseline_id".into(),
                Value::String(baseline_id.into()),
            );
            object.remove("unavailable_since_baseline_id");
            changed += tx.execute(
                "UPDATE entities SET status='active',version=version+1,data_json=?3,
                    updated_at=?4,updated_by=?5
                 WHERE id=?1 AND project_id=?2 AND entity_type=?6",
                params![
                    id,
                    project_id,
                    bounded_json(&data, MAX_JSON_BYTES, "reactivated entity data")?,
                    now,
                    command.actor.id,
                    entity_type
                ],
            )?;
        }
    }
    Ok(changed)
}

#[allow(clippy::too_many_arguments)]
fn mark_unobserved_code_intelligence(
    tx: &Transaction<'_>,
    project_id: &str,
    command: &CommandContext,
    repository_id: &str,
    baseline_id: &str,
    observed_code_ids: &HashSet<String>,
    observed_test_ids: &HashSet<String>,
    now: &str,
) -> Result<usize> {
    let mut statement = tx.prepare(
        "SELECT e.id,e.entity_type,e.data_json
         FROM entities e
         WHERE e.project_id=?1 AND e.status<>'unavailable' AND (
           (e.entity_type='code_entity' AND EXISTS(
             SELECT 1 FROM code_entities ce
             WHERE ce.entity_id=e.id AND ce.repository_id=?2
           )) OR
           (e.entity_type='test' AND EXISTS(
             SELECT 1 FROM tests t
             WHERE t.entity_id=e.id AND t.repository_id=?2
           ))
         )
         ORDER BY e.id",
    )?;
    let candidates = statement
        .query_map(params![project_id, repository_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    drop(statement);

    let mut changed = 0_usize;
    for (id, entity_type, raw_data) in candidates {
        let observed = match entity_type.as_str() {
            "code_entity" => observed_code_ids.contains(&id),
            "test" => observed_test_ids.contains(&id),
            _ => true,
        };
        if observed {
            continue;
        }
        let mut data: Value = serde_json::from_str(&raw_data)?;
        let object = data.as_object_mut().ok_or_else(|| {
            CoreError::Conflict(format!(
                "{entity_type} {id} has non-object current data; run integrity diagnostics"
            ))
        })?;
        object.insert("presence".into(), Value::String("unavailable".into()));
        object.insert(
            "unavailable_since_baseline_id".into(),
            Value::String(baseline_id.into()),
        );
        changed += tx.execute(
            "UPDATE entities SET status='unavailable',version=version+1,data_json=?3,
                updated_at=?4,updated_by=?5
             WHERE id=?1 AND project_id=?2 AND entity_type=?6 AND status<>'unavailable'",
            params![
                id,
                project_id,
                bounded_json(&data, MAX_JSON_BYTES, "unavailable entity data")?,
                now,
                command.actor.id,
                entity_type
            ],
        )?;
        if entity_type == "code_entity" {
            tx.execute(
                "UPDATE code_entity_aliases SET last_seen_baseline_id=?3,
                    retired_at_baseline_id=?3
                 WHERE repository_id=?1 AND code_entity_id=?2
                   AND alias_kind='path' AND retired_at_baseline_id IS NULL",
                params![repository_id, id, baseline_id],
            )?;
        }
    }
    Ok(changed)
}

fn find_symbol_id(
    connection: &Connection,
    repository_id: &str,
    file_id: &str,
    qualified_name: &str,
) -> Result<Option<String>> {
    connection
        .query_row(
            "SELECT entity_id FROM code_entities WHERE repository_id=?1 AND entity_kind='symbol'
             AND stable_key LIKE ?2 ESCAPE '\\' LIMIT 1",
            params![
                repository_id,
                format!(
                    "symbol:{}:%:{}",
                    escape_like(file_id),
                    escape_like(qualified_name)
                )
            ],
            |row| row.get(0),
        )
        .optional()
        .map_err(Into::into)
}

fn link_change_sets_to_files(
    tx: &Transaction<'_>,
    project_id: &str,
    command: &CommandContext,
    repository_id: &str,
    baseline_id: &str,
    file_ids: &HashMap<String, String>,
    run_id: &str,
) -> Result<()> {
    let mut statement = tx.prepare(
        "SELECT cs.entity_id,fc.old_path,fc.new_path FROM change_sets cs
         JOIN change_set_file_changes fc ON fc.change_set_id=cs.entity_id
         WHERE cs.project_id=?1 AND cs.repository_id=?2 AND cs.baseline_id=?3",
    )?;
    let rows = statement
        .query_map(params![project_id, repository_id, baseline_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, Option<String>>(1)?,
                row.get::<_, String>(2)?,
            ))
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    drop(statement);
    for (change_set_id, old_path, new_path) in rows {
        let code_id = file_ids
            .get(&new_path)
            .or_else(|| old_path.as_ref().and_then(|path| file_ids.get(path)))
            .cloned()
            .or(resolve_file_entity_id(
                tx,
                repository_id,
                baseline_id,
                &new_path,
                old_path.as_deref(),
            )?);
        if let Some(code_id) = code_id {
            insert_relationship_if_absent(
                tx,
                project_id,
                command,
                &change_set_id,
                "change_set",
                &code_id,
                "code_entity",
                "modifies",
                OriginKind::Deterministic,
                &[baseline_id.to_owned(), run_id.to_owned()],
            )?;
        }
    }
    Ok(())
}

pub(crate) fn link_change_set_to_known_code_entities(
    tx: &Transaction<'_>,
    project_id: &str,
    command: &CommandContext,
    change_set_id: &str,
    repository_id: &str,
    baseline_id: &str,
) -> Result<()> {
    let analysis_run_id: Option<String> = tx
        .query_row(
            "SELECT entity_id FROM analysis_runs
             WHERE project_id=?1 AND repository_id=?2 AND baseline_id=?3
             ORDER BY completed_at DESC,entity_id DESC LIMIT 1",
            params![project_id, repository_id, baseline_id],
            |row| row.get(0),
        )
        .optional()?;
    let Some(analysis_run_id) = analysis_run_id else {
        return Ok(());
    };
    let mut statement = tx.prepare(
        "SELECT old_path,new_path FROM change_set_file_changes
         WHERE change_set_id=?1 ORDER BY ordinal",
    )?;
    let changes = statement
        .query_map([change_set_id], |row| {
            Ok((row.get::<_, Option<String>>(0)?, row.get::<_, String>(1)?))
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    drop(statement);
    for (old_path, new_path) in changes {
        if let Some(code_entity_id) = resolve_file_entity_id(
            tx,
            repository_id,
            baseline_id,
            &new_path,
            old_path.as_deref(),
        )? {
            insert_relationship_if_absent(
                tx,
                project_id,
                command,
                change_set_id,
                "change_set",
                &code_entity_id,
                "code_entity",
                "modifies",
                OriginKind::Deterministic,
                &[baseline_id.to_owned(), analysis_run_id.clone()],
            )?;
        }
    }
    Ok(())
}

fn resolve_file_entity_id(
    connection: &Connection,
    repository_id: &str,
    baseline_id: &str,
    new_path: &str,
    old_path: Option<&str>,
) -> Result<Option<String>> {
    for path in std::iter::once(new_path).chain(old_path) {
        if let Some(id) = connection
            .query_row(
                "SELECT ceo.code_entity_id FROM code_entity_observations ceo
                 JOIN code_entities ce ON ce.entity_id=ceo.code_entity_id
                 WHERE ce.repository_id=?1 AND ce.entity_kind='file'
                   AND ceo.baseline_id=?2 AND ceo.source_path=?3
                 ORDER BY ceo.observed_at DESC LIMIT 1",
                params![repository_id, baseline_id, path],
                |row| row.get(0),
            )
            .optional()?
        {
            return Ok(Some(id));
        }
        if let Some(id) = connection
            .query_row(
                "SELECT cea.code_entity_id FROM code_entity_aliases cea
                 JOIN code_entities ce ON ce.entity_id=cea.code_entity_id
                 WHERE cea.repository_id=?1 AND cea.alias_kind='path' AND cea.alias_value=?2
                   AND cea.retired_at_baseline_id=?3 AND ce.entity_kind='file'
                 ORDER BY cea.code_entity_id LIMIT 1",
                params![repository_id, path, baseline_id],
                |row| row.get(0),
            )
            .optional()?
        {
            return Ok(Some(id));
        }
        if let Some(id) = connection
            .query_row(
                "SELECT cea.code_entity_id FROM code_entity_aliases cea
                 JOIN code_entities ce ON ce.entity_id=cea.code_entity_id
                 WHERE cea.repository_id=?1 AND cea.alias_kind='path' AND cea.alias_value=?2
                   AND cea.retired_at_baseline_id IS NULL AND ce.entity_kind='file'",
                params![repository_id, path],
                |row| row.get(0),
            )
            .optional()?
        {
            return Ok(Some(id));
        }
    }
    Ok(None)
}

#[allow(clippy::too_many_arguments)]
fn insert_relationship_if_absent(
    tx: &Transaction<'_>,
    project_id: &str,
    command: &CommandContext,
    source_id: &str,
    source_type: &str,
    target_id: &str,
    target_type: &str,
    relation_type: &str,
    origin: OriginKind,
    direct_sources: &[String],
) -> Result<String> {
    crate::development::validate_development_relationship_pair(
        source_type,
        relation_type,
        target_type,
    )?;
    require_typed_entity(tx, project_id, source_id, source_type)?;
    require_typed_entity(tx, project_id, target_id, target_type)?;
    if let Some(id) = tx
        .query_row(
            "SELECT id FROM relationships WHERE project_id=?1 AND relation_type=?2
             AND source_entity_id=?3 AND target_entity_id=?4",
            params![project_id, relation_type, source_id, target_id],
            |row| row.get(0),
        )
        .optional()?
    {
        return Ok(id);
    }
    let id = new_id();
    let now = Utc::now().to_rfc3339();
    tx.execute(
        "INSERT INTO relationships(id,project_id,relation_type,relation_version,source_entity_id,
            source_entity_type,target_entity_id,target_entity_type,status,origin_type,actor_id,
            confidence,review_state,direct_source_ids_json,supersedes_id,created_at,updated_at)
         VALUES(?1,?2,?3,1,?4,?5,?6,?7,'active',?8,?9,NULL,?10,?11,NULL,?12,?12)",
        params![
            id,
            project_id,
            relation_type,
            source_id,
            source_type,
            target_id,
            target_type,
            origin.as_str(),
            command.actor.id,
            if origin == OriginKind::Deterministic {
                "unreviewed"
            } else {
                "accepted"
            },
            bounded_json(
                &json!(direct_sources),
                MAX_JSON_BYTES,
                "relationship direct sources"
            )?,
            now
        ],
    )?;
    Ok(id)
}

fn require_typed_entity(
    connection: &Connection,
    project_id: &str,
    id: &str,
    expected: &str,
) -> Result<()> {
    let actual: Option<String> = connection
        .query_row(
            "SELECT entity_type FROM entities WHERE id=?1 AND project_id=?2",
            params![id, project_id],
            |row| row.get(0),
        )
        .optional()?;
    match actual.as_deref() {
        Some(value) if value == expected => Ok(()),
        Some(value) => Err(CoreError::Validation(format!(
            "entity {id} is {value}, expected {expected}"
        ))),
        None => Err(CoreError::NotFound(id.into())),
    }
}

fn validate_analysis_limits(limits: &AnalysisLimits) -> Result<()> {
    if limits.max_files == 0
        || limits.max_files > MAX_FILES_HARD
        || limits.max_file_bytes == 0
        || limits.max_file_bytes > MAX_FILE_BYTES_HARD
        || limits.max_total_bytes == 0
        || limits.max_total_bytes > MAX_TOTAL_BYTES_HARD
        || limits.max_entities == 0
        || limits.max_entities > MAX_ENTITIES_HARD
        || limits.max_file_bytes > limits.max_total_bytes
    {
        return Err(CoreError::Validation(format!(
            "analysis limits must be positive and within files={MAX_FILES_HARD}, file_bytes={MAX_FILE_BYTES_HARD}, total_bytes={MAX_TOTAL_BYTES_HARD}, entities={MAX_ENTITIES_HARD}"
        )));
    }
    Ok(())
}

fn analysis_source_fingerprint(baseline: &RepositoryBaselineRecord) -> String {
    sha256_hex(
        format!(
            "{}\0{}\0{}\0{}",
            baseline.repository_id,
            baseline.head_oid.as_deref().unwrap_or("unborn"),
            baseline.worktree_fingerprint,
            ANALYZER_BUNDLE_VERSION
        )
        .as_bytes(),
    )
}

fn existing_analysis_run(
    connection: &Connection,
    project_id: &str,
    repository_id: &str,
    baseline_id: &str,
    source_fingerprint: &str,
) -> Result<Option<String>> {
    connection
        .query_row(
            "SELECT entity_id FROM analysis_runs WHERE project_id=?1 AND repository_id=?2
             AND baseline_id=?3 AND source_fingerprint=?4 AND analyzer_bundle_version=?5
             AND output_schema_version=?6",
            params![
                project_id,
                repository_id,
                baseline_id,
                source_fingerprint,
                ANALYZER_BUNDLE_VERSION,
                OUTPUT_SCHEMA_VERSION
            ],
            |row| row.get(0),
        )
        .optional()
        .map_err(Into::into)
}

fn is_current_repository_baseline(
    connection: &Connection,
    repository_id: &str,
    baseline_id: &str,
) -> Result<bool> {
    let current: Option<String> = connection
        .query_row(
            "SELECT current_baseline_id FROM repository_reconciliations
             WHERE repository_id=?1 ORDER BY observed_at DESC,id DESC LIMIT 1",
            [repository_id],
            |row| row.get(0),
        )
        .optional()?;
    Ok(current.as_deref() == Some(baseline_id))
}

fn enforce_analyzer_cache_budget(connection: &Connection) -> Result<()> {
    let mut total: i64 = connection.query_row(
        "SELECT COALESCE(sum(byte_size),0) FROM analyzer_cache",
        [],
        |row| row.get(0),
    )?;
    while total > ANALYZER_CACHE_MAX_BYTES {
        let removed = connection.execute(
            "DELETE FROM analyzer_cache WHERE rowid IN (
                SELECT rowid FROM analyzer_cache
                ORDER BY last_accessed_at,created_at,content_sha256 LIMIT 128
             )",
            [],
        )?;
        if removed == 0 {
            break;
        }
        total = connection.query_row(
            "SELECT COALESCE(sum(byte_size),0) FROM analyzer_cache",
            [],
            |row| row.get(0),
        )?;
    }
    Ok(())
}

fn read_cache(
    connection: &Connection,
    hash: &str,
    language: &str,
) -> Result<Option<ParsedFileOutput>> {
    let raw: Option<String> = connection
        .query_row(
            "SELECT output_json FROM analyzer_cache WHERE content_sha256=?1 AND language=?2
             AND analyzer_id='code-intelligence-file' AND analyzer_version=?3
             AND output_schema_version=?4",
            params![
                hash,
                language,
                FILE_ANALYSIS_CACHE_VERSION,
                OUTPUT_SCHEMA_VERSION
            ],
            |row| row.get(0),
        )
        .optional()?;
    if raw.is_some() {
        connection.execute(
            "UPDATE analyzer_cache SET last_accessed_at=?5
             WHERE content_sha256=?1 AND language=?2
               AND analyzer_id='code-intelligence-file' AND analyzer_version=?3
               AND output_schema_version=?4",
            params![
                hash,
                language,
                FILE_ANALYSIS_CACHE_VERSION,
                OUTPUT_SCHEMA_VERSION,
                Utc::now().to_rfc3339()
            ],
        )?;
    }
    raw.map(|value| serde_json::from_str(&value).map_err(Into::into))
        .transpose()
}

fn read_analyzer_executions(
    connection: &Connection,
    run_id: &str,
) -> Result<Vec<AnalyzerExecutionRecord>> {
    let mut statement = connection.prepare(
        "SELECT analyzer_id,analyzer_version,contract_version,output_schema_version,status,
            files_seen,outputs_created,cache_hits,cache_misses,duration_ms,diagnostic_code
         FROM analyzer_executions WHERE analysis_run_id=?1 ORDER BY ordinal",
    )?;
    statement
        .query_map([run_id], |row| {
            Ok(AnalyzerExecutionRecord {
                analyzer_id: row.get(0)?,
                analyzer_version: row.get(1)?,
                contract_version: row.get(2)?,
                output_schema_version: row.get(3)?,
                status: row.get(4)?,
                files_seen: row.get::<_, i64>(5)? as usize,
                outputs_created: row.get::<_, i64>(6)? as usize,
                cache_hits: row.get::<_, i64>(7)? as usize,
                cache_misses: row.get::<_, i64>(8)? as usize,
                duration_ms: row.get::<_, i64>(9)? as u64,
                diagnostic_code: row.get(10)?,
            })
        })?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(Into::into)
}

fn read_limitations(
    connection: &Connection,
    run_id: &str,
) -> Result<Vec<AnalysisLimitationRecord>> {
    let mut statement = connection.prepare(
        "SELECT id,source_path,analyzer_id,code,severity,message FROM analysis_limitations
         WHERE analysis_run_id=?1 ORDER BY source_path,code,id",
    )?;
    statement
        .query_map([run_id], |row| {
            Ok(AnalysisLimitationRecord {
                id: row.get(0)?,
                source_path: row.get(1)?,
                analyzer_id: row.get(2)?,
                code: row.get(3)?,
                severity: row.get(4)?,
                message: row.get(5)?,
            })
        })?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(Into::into)
}

fn latest_code_observation(
    connection: &Connection,
    id: &str,
) -> Result<Option<CodeObservationRecord>> {
    connection
        .query_row(
            "SELECT id,analysis_run_id,baseline_id,code_entity_id,source_path,qualified_name,
                content_sha256,signature_sha256,start_line,start_column,end_line,end_column,visibility,
                observation_status,analyzer_id,analyzer_version,output_schema_version,details_json,observed_at
             FROM code_entity_observations WHERE code_entity_id=?1
             ORDER BY observed_at DESC,id DESC LIMIT 1",
            [id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, Option<String>>(5)?,
                    row.get::<_, String>(6)?,
                    row.get::<_, String>(7)?,
                    row.get::<_, Option<i64>>(8)?,
                    row.get::<_, Option<i64>>(9)?,
                    row.get::<_, Option<i64>>(10)?,
                    row.get::<_, Option<i64>>(11)?,
                    row.get::<_, Option<String>>(12)?,
                    row.get::<_, String>(13)?,
                    row.get::<_, String>(14)?,
                    row.get::<_, String>(15)?,
                    row.get::<_, i64>(16)?,
                    row.get::<_, String>(17)?,
                    row.get::<_, String>(18)?,
                ))
            },
        )
        .optional()?
        .map(|r| {
            Ok(CodeObservationRecord {
                id: r.0,
                analysis_run_id: r.1,
                baseline_id: r.2,
                code_entity_id: r.3,
                source_path: r.4,
                qualified_name: r.5,
                content_sha256: r.6,
                signature_sha256: r.7,
                start_line: r.8.map(|v| v as u64),
                start_column: r.9.map(|v| v as u64),
                end_line: r.10.map(|v| v as u64),
                end_column: r.11.map(|v| v as u64),
                visibility: r.12,
                observation_status: r.13,
                analyzer_id: r.14,
                analyzer_version: r.15,
                output_schema_version: r.16,
                details: serde_json::from_str(&r.17)?,
                observed_at: r.18,
            })
        })
        .transpose()
}

fn latest_test_observation(
    connection: &Connection,
    id: &str,
) -> Result<Option<TestObservationRecord>> {
    connection
        .query_row(
            "SELECT id,analysis_run_id,baseline_id,test_id,code_entity_id,source_path,qualified_name,
                content_sha256,start_line,end_line,analyzer_id,analyzer_version,details_json,observed_at
             FROM test_observations WHERE test_id=?1 ORDER BY observed_at DESC,id DESC LIMIT 1",
            [id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, String>(6)?,
                    row.get::<_, String>(7)?,
                    row.get::<_, i64>(8)?,
                    row.get::<_, i64>(9)?,
                    row.get::<_, String>(10)?,
                    row.get::<_, String>(11)?,
                    row.get::<_, String>(12)?,
                    row.get::<_, String>(13)?,
                ))
            },
        )
        .optional()?
        .map(|r| {
            Ok(TestObservationRecord {
                id: r.0,
                analysis_run_id: r.1,
                baseline_id: r.2,
                test_id: r.3,
                code_entity_id: r.4,
                source_path: r.5,
                qualified_name: r.6,
                content_sha256: r.7,
                start_line: r.8 as u64,
                end_line: r.9 as u64,
                analyzer_id: r.10,
                analyzer_version: r.11,
                details: serde_json::from_str(&r.12)?,
                observed_at: r.13,
            })
        })
        .transpose()
}

fn support_language(language: &str) -> Option<SupportLang> {
    match language {
        "rust" => Some(SupportLang::Rust),
        "javascript" => Some(SupportLang::JavaScript),
        "typescript" => Some(SupportLang::TypeScript),
        "tsx" => Some(SupportLang::Tsx),
        "python" => Some(SupportLang::Python),
        "json" => Some(SupportLang::Json),
        _ => None,
    }
}

fn detect_language(path: &str) -> Option<String> {
    let lower = path.to_ascii_lowercase();
    let language = if lower.ends_with(".rs") {
        "rust"
    } else if lower.ends_with(".tsx") {
        "tsx"
    } else if lower.ends_with(".ts") || lower.ends_with(".mts") || lower.ends_with(".cts") {
        "typescript"
    } else if lower.ends_with(".js")
        || lower.ends_with(".jsx")
        || lower.ends_with(".mjs")
        || lower.ends_with(".cjs")
    {
        "javascript"
    } else if lower.ends_with(".py") {
        "python"
    } else if lower.ends_with(".json") {
        "json"
    } else {
        return None;
    };
    Some(language.into())
}

fn symbol_kind<'a>(language: &str, node_kind: &'a str) -> Option<&'a str> {
    let supported = match language {
        "rust" => matches!(
            node_kind,
            "function_item"
                | "struct_item"
                | "enum_item"
                | "trait_item"
                | "impl_item"
                | "mod_item"
                | "type_item"
                | "const_item"
                | "static_item"
                | "macro_definition"
        ),
        "javascript" | "typescript" | "tsx" => matches!(
            node_kind,
            "function_declaration"
                | "generator_function_declaration"
                | "class_declaration"
                | "method_definition"
                | "interface_declaration"
                | "type_alias_declaration"
                | "enum_declaration"
        ),
        "python" => matches!(node_kind, "function_definition" | "class_definition"),
        _ => false,
    };
    supported.then_some(node_kind)
}

fn qualified_name<D: ast_grep_core::Doc>(node: &ast_grep_core::Node<'_, D>, name: &str) -> String {
    let mut parts = node
        .ancestors()
        .filter_map(|ancestor| {
            let kind = ancestor.kind();
            if kind.as_ref() == "impl_item" {
                ancestor
                    .field("type")
                    .map(|node| bounded_fragment(&node.text(), 200))
            } else if matches!(
                kind.as_ref(),
                "class_declaration"
                    | "class_definition"
                    | "trait_item"
                    | "mod_item"
                    | "function_item"
                    | "function_definition"
                    | "function_declaration"
                    | "generator_function_declaration"
                    | "method_definition"
            ) {
                ancestor
                    .field("name")
                    .map(|node| bounded_fragment(&node.text(), 200))
            } else {
                None
            }
        })
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>();
    parts.reverse();
    parts.push(name.to_owned());
    parts.join("::")
}

fn is_structural_test<D: ast_grep_core::Doc>(
    language: &str,
    node: &ast_grep_core::Node<'_, D>,
    source: &str,
    name: &str,
) -> bool {
    match language {
        "rust" => {
            let start = node.range().start;
            let prefix = &source[start.saturating_sub(512)..start];
            prefix.contains("#[test]")
                || prefix.contains("#[tokio::test]")
                || prefix.contains("#[rstest]")
        }
        "python" => {
            name.starts_with("test_")
                || node.ancestors().any(|ancestor| {
                    ancestor.kind().as_ref() == "class_definition"
                        && ancestor
                            .field("name")
                            .is_some_and(|name| name.text().starts_with("Test"))
                })
        }
        _ => false,
    }
}

fn test_framework(language: &str, source: &str, start: usize) -> &'static str {
    match language {
        "rust" => {
            let prefix = &source[start.saturating_sub(512)..start];
            if prefix.contains("#[tokio::test]") {
                "tokio"
            } else if prefix.contains("#[rstest]") {
                "rstest"
            } else {
                "rust-test"
            }
        }
        "python" => "pytest-or-unittest",
        _ => "unknown",
    }
}

fn detect_visibility(language: &str, name: &str, text: &str) -> Option<String> {
    match language {
        "rust" => Some(
            if text.trim_start().starts_with("pub") {
                "public"
            } else {
                "private"
            }
            .into(),
        ),
        "javascript" | "typescript" | "tsx" => Some(
            if text.trim_start().starts_with("export") {
                "exported"
            } else {
                "module"
            }
            .into(),
        ),
        "python" => Some(
            if name.starts_with('_') {
                "private"
            } else {
                "module"
            }
            .into(),
        ),
        _ => None,
    }
}

fn analyze_dependencies(path: &str, source: &str) -> Result<Vec<ParsedDependency>> {
    let name = Path::new(path)
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or_default();
    let mut dependencies = Vec::new();
    if name == "Cargo.toml" || name == "pyproject.toml" {
        if let Ok(value) = toml::from_str::<toml::Value>(source) {
            collect_toml_dependencies(
                &value,
                if name == "Cargo.toml" {
                    "cargo"
                } else {
                    "python"
                },
                &mut dependencies,
            );
            if name == "pyproject.toml"
                && let Some(items) = value
                    .get("project")
                    .and_then(|value| value.get("dependencies"))
                    .and_then(toml::Value::as_array)
            {
                for item in items.iter().filter_map(toml::Value::as_str) {
                    let (dependency, requirement) = split_python_requirement(item);
                    if !dependency.is_empty() {
                        dependencies.push(ParsedDependency {
                            ecosystem: "python".into(),
                            scope: "runtime".into(),
                            name: dependency,
                            requirement,
                        });
                    }
                }
            }
        }
    } else if name == "package.json" {
        if let Ok(value) = serde_json::from_str::<Value>(source) {
            for (field, scope) in [
                ("dependencies", "runtime"),
                ("devDependencies", "development"),
                ("peerDependencies", "peer"),
                ("optionalDependencies", "optional"),
            ] {
                if let Some(map) = value.get(field).and_then(Value::as_object) {
                    for (dependency, requirement) in map {
                        dependencies.push(ParsedDependency {
                            ecosystem: "npm".into(),
                            scope: scope.into(),
                            name: dependency.clone(),
                            requirement: sanitize_requirement(
                                requirement.as_str().unwrap_or("<structured>"),
                            ),
                        });
                    }
                }
            }
        }
    } else if matches!(name, "requirements.txt" | "requirements-dev.txt") {
        for line in source
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty() && !line.starts_with('#') && !line.starts_with('-'))
        {
            let (dependency, requirement) = split_python_requirement(line);
            if !dependency.is_empty() {
                dependencies.push(ParsedDependency {
                    ecosystem: "python".into(),
                    scope: if name.contains("dev") {
                        "development"
                    } else {
                        "runtime"
                    }
                    .into(),
                    name: dependency,
                    requirement,
                });
            }
        }
    }
    dependencies.sort_by(|a, b| {
        (&a.ecosystem, &a.scope, &a.name, &a.requirement).cmp(&(
            &b.ecosystem,
            &b.scope,
            &b.name,
            &b.requirement,
        ))
    });
    dependencies.dedup_by(|a, b| {
        a.ecosystem == b.ecosystem
            && a.scope == b.scope
            && a.name == b.name
            && a.requirement == b.requirement
    });
    Ok(dependencies)
}

fn collect_toml_dependencies(
    value: &toml::Value,
    ecosystem: &str,
    out: &mut Vec<ParsedDependency>,
) {
    let Some(table) = value.as_table() else {
        return;
    };
    for (key, value) in table {
        if matches!(
            key.as_str(),
            "dependencies" | "dev-dependencies" | "build-dependencies"
        ) {
            if let Some(dependencies) = value.as_table() {
                let scope = match key.as_str() {
                    "dev-dependencies" => "development",
                    "build-dependencies" => "build",
                    _ => "runtime",
                };
                for (name, specification) in dependencies {
                    out.push(ParsedDependency {
                        ecosystem: ecosystem.into(),
                        scope: scope.into(),
                        name: name.clone(),
                        requirement: toml_requirement(specification),
                    });
                }
            }
        } else {
            collect_toml_dependencies(value, ecosystem, out);
        }
    }
}

fn toml_requirement(value: &toml::Value) -> String {
    match value {
        toml::Value::String(value) => sanitize_requirement(value),
        toml::Value::Table(table) => {
            let mut safe = toml::map::Map::new();
            for key in [
                "version",
                "path",
                "git",
                "branch",
                "tag",
                "rev",
                "workspace",
                "optional",
                "features",
            ] {
                if let Some(value) = table.get(key) {
                    if key == "git" {
                        safe.insert(key.into(), toml::Value::String("<external-source>".into()));
                    } else {
                        safe.insert(key.into(), value.clone());
                    }
                }
            }
            toml::Value::Table(safe)
                .to_string()
                .chars()
                .take(1000)
                .collect()
        }
        other => other.to_string().chars().take(1000).collect(),
    }
}

fn split_python_requirement(value: &str) -> (String, String) {
    let clean = value.split(';').next().unwrap_or(value).trim();
    let end = clean
        .find(['<', '>', '=', '!', '~', '[', ' '])
        .unwrap_or(clean.len());
    (
        clean[..end].trim().to_owned(),
        sanitize_requirement(clean[end..].trim()),
    )
}

fn sanitize_requirement(value: &str) -> String {
    let bounded = value.trim().chars().take(1000).collect::<String>();
    if bounded.contains("://") && bounded.contains('@') {
        "<redacted-uri>".into()
    } else {
        bounded
    }
}

fn analyze_configuration(path: &str, source: &str) -> Option<ParsedConfiguration> {
    let lower = path.to_ascii_lowercase();
    let name = Path::new(path)
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or_default();
    let format = if lower.ends_with(".json") {
        Some("json")
    } else if lower.ends_with(".toml") {
        Some("toml")
    } else if lower.ends_with(".yaml") || lower.ends_with(".yml") {
        Some("yaml")
    } else if name.starts_with(".env") {
        Some("dotenv")
    } else if name == "Dockerfile" || name.ends_with(".config.js") || name.ends_with(".config.ts") {
        Some("executable-config")
    } else {
        None
    }?;
    let (parse_status, keys) = match format {
        "json" => match serde_json::from_str::<Value>(source) {
            Ok(Value::Object(map)) => ("valid", map.keys().take(200).cloned().collect()),
            Ok(_) => ("valid", Vec::new()),
            Err(_) => ("invalid", Vec::new()),
        },
        "toml" => match toml::from_str::<toml::Value>(source) {
            Ok(toml::Value::Table(map)) => ("valid", map.keys().take(200).cloned().collect()),
            Ok(_) => ("valid", Vec::new()),
            Err(_) => ("invalid", Vec::new()),
        },
        "dotenv" => (
            "keys_only",
            source
                .lines()
                .filter_map(|line| line.split_once('=').map(|(key, _)| key.trim().to_owned()))
                .filter(|key| !key.is_empty())
                .take(200)
                .collect(),
        ),
        _ => ("recognized_unparsed", Vec::new()),
    };
    Some(ParsedConfiguration {
        format: format.into(),
        parse_status: parse_status.into(),
        top_level_keys: keys,
        sensitive_values_omitted: format == "dotenv",
    })
}

fn empty_output(language: Option<String>) -> ParsedFileOutput {
    ParsedFileOutput {
        language,
        parser_had_error: false,
        symbols: Vec::new(),
        tests: Vec::new(),
        dependencies: Vec::new(),
        configuration: None,
    }
}

fn deduplicate_parsed(output: &mut ParsedFileOutput) {
    output.symbols.sort_by(|a, b| {
        (&a.qualified_name, &a.kind, a.start_line).cmp(&(&b.qualified_name, &b.kind, b.start_line))
    });
    output
        .symbols
        .dedup_by(|a, b| a.qualified_name == b.qualified_name && a.kind == b.kind);
    output.tests.sort_by(|a, b| {
        (&a.framework, &a.qualified_name, a.start_line).cmp(&(
            &b.framework,
            &b.qualified_name,
            b.start_line,
        ))
    });
    output.tests.dedup_by(|a, b| {
        a.framework == b.framework
            && a.qualified_name == b.qualified_name
            && a.start_line == b.start_line
    });
}

fn estimate_entity_count(prepared: &PreparedAnalysis) -> usize {
    prepared.files.len()
        + prepared
            .files
            .iter()
            .map(|file| {
                file.output.symbols.len()
                    + file.output.tests.len()
                    + file.output.dependencies.len()
                    + usize::from(file.output.configuration.is_some())
            })
            .sum::<usize>()
}

fn expected_unique_counts(prepared: &PreparedAnalysis) -> (usize, usize) {
    let mut code = BTreeSet::new();
    let mut tests = BTreeSet::new();
    for file in &prepared.files {
        code.insert(format!("file:{}", file.path));
        for symbol in &file.output.symbols {
            code.insert(format!(
                "symbol:{}:{}:{}",
                file.path, symbol.kind, symbol.qualified_name
            ));
        }
        for dependency in &file.output.dependencies {
            code.insert(format!(
                "dependency:{}:{}:{}",
                dependency.ecosystem, dependency.scope, dependency.name
            ));
        }
        if file.output.configuration.is_some() {
            code.insert(format!("configuration:{}", file.path));
        }
        for test in &file.output.tests {
            tests.insert(format!(
                "test:{}:{}:{}",
                file.path, test.framework, test.qualified_name
            ));
        }
    }
    (code.len(), tests.len())
}

fn normalized_signature(text: &str) -> String {
    let head = text.split(['{', '\n']).next().unwrap_or(text);
    head.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .take(2000)
        .collect()
}

fn first_quoted_argument(text: &str) -> Option<String> {
    let open = text.find('(')?;
    let rest = &text[open + 1..];
    let single = rest.find('\'');
    let double = rest.find('"');
    let (quote, delimiter) = match (single, double) {
        (Some(a), Some(b)) if a <= b => (a, '\''),
        (Some(_), Some(b)) => (b, '"'),
        (Some(a), None) => (a, '\''),
        (None, Some(b)) => (b, '"'),
        (None, None) => return None,
    };
    let value = &rest[quote + 1..];
    let end = value.find(delimiter)?;
    Some(value[..end].chars().take(300).collect())
}

fn bounded_fragment(value: &str, max: usize) -> String {
    value.trim().chars().take(max).collect()
}

fn short_id(value: &str) -> &str {
    value.get(..8).unwrap_or(value)
}

fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

fn escape_like(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
}

fn validate_json(value: &Value, label: &str) -> Result<()> {
    bounded_json(value, MAX_JSON_BYTES, label).map(|_| ())
}

fn validate_text(value: &str, max: usize, label: &str) -> Result<()> {
    if value.chars().count() > max {
        Err(CoreError::Validation(format!(
            "{label} exceeds {max} characters"
        )))
    } else {
        Ok(())
    }
}

fn ensure_unique<'a>(items: impl Iterator<Item = &'a str>, message: &str) -> Result<()> {
    let mut seen = HashSet::new();
    if items.into_iter().all(|item| seen.insert(item)) {
        Ok(())
    } else {
        Err(CoreError::Validation(message.into()))
    }
}

fn validate_test_run_consistency(outcome: TestOutcome, results: &[TestResultInput]) -> Result<()> {
    if outcome == TestOutcome::Passed
        && results.iter().any(|result| {
            matches!(
                result.outcome,
                TestOutcome::Failed | TestOutcome::Error | TestOutcome::Cancelled
            )
        })
    {
        return Err(CoreError::Validation(
            "a passed TestRun cannot contain failed, error, or cancelled Test results".into(),
        ));
    }
    Ok(())
}

pub(crate) fn append_code_intelligence_integrity_issues(
    connection: &Connection,
    project_id: &str,
    report: &mut IntegrityReport,
) -> Result<()> {
    for (kind, table) in [
        ("analysis_run", "analysis_runs"),
        ("code_entity", "code_entities"),
        ("test", "tests"),
        ("test_run", "test_runs"),
    ] {
        let sql = format!(
            "SELECT e.id FROM entities e LEFT JOIN {table} d ON d.entity_id=e.id
             WHERE e.project_id=?1 AND e.entity_type=?2 AND d.entity_id IS NULL"
        );
        let mut statement = connection.prepare(&sql)?;
        let ids = statement
            .query_map(params![project_id, kind], |row| row.get::<_, String>(0))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        for id in ids {
            report.issues.push(IntegrityIssue {
                code: "missing_code_intelligence_detail".into(),
                path_or_id: id,
                guidance: format!(
                    "Restore the normalized {kind} row from a verified backup; do not infer analyzer output."
                ),
            });
        }
    }
    let mut statement = connection.prepare(
        "SELECT ar.entity_id FROM analysis_runs ar
         WHERE ar.project_id=?1 AND (ar.code_entity_count<>(SELECT count(DISTINCT code_entity_id)
             FROM code_entity_observations o WHERE o.analysis_run_id=ar.entity_id)
          OR ar.test_count<>(SELECT count(DISTINCT test_id) FROM test_observations t
             WHERE t.analysis_run_id=ar.entity_id)
          OR ar.limitation_count<>(SELECT count(*) FROM analysis_limitations l
             WHERE l.analysis_run_id=ar.entity_id))",
    )?;
    let ids = statement
        .query_map([project_id], |row| row.get::<_, String>(0))?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    for id in ids {
        report.issues.push(IntegrityIssue {
            code: "analysis_count_mismatch".into(),
            path_or_id: id,
            guidance: "Rebuild the affected analysis from its immutable RepositoryBaseline; do not edit counts manually.".into(),
        });
    }
    let mut statement = connection.prepare(
        "SELECT id FROM (
           SELECT ce.entity_id AS id FROM code_entities ce
             JOIN entities e ON e.id=ce.entity_id
             WHERE ce.project_id=?1 AND ce.entity_kind='file' AND e.status='active'
               AND NOT EXISTS(SELECT 1 FROM code_entity_aliases a
                 WHERE a.code_entity_id=ce.entity_id AND a.alias_kind='path'
                   AND a.retired_at_baseline_id IS NULL)
           UNION
           SELECT a.code_entity_id FROM code_entity_aliases a
             JOIN code_entities ce ON ce.entity_id=a.code_entity_id
             JOIN repository_baselines first_rb ON first_rb.entity_id=a.first_seen_baseline_id
             JOIN repository_baselines last_rb ON last_rb.entity_id=a.last_seen_baseline_id
             LEFT JOIN repository_baselines retired_rb ON retired_rb.entity_id=a.retired_at_baseline_id
             WHERE ce.project_id=?1 AND (ce.repository_id<>a.repository_id
               OR first_rb.repository_id<>a.repository_id
               OR last_rb.repository_id<>a.repository_id
               OR (retired_rb.entity_id IS NOT NULL AND retired_rb.repository_id<>a.repository_id))
         ) ORDER BY id",
    )?;
    let ids = statement
        .query_map([project_id], |row| row.get::<_, String>(0))?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    for id in ids {
        report.issues.push(IntegrityIssue {
            code: "invalid_code_entity_alias_timeline".into(),
            path_or_id: id,
            guidance: "Rebuild temporal file aliases from immutable code observations and CP4 rename evidence; do not reuse an unrelated path identity.".into(),
        });
    }
    let cache_bytes: i64 = connection.query_row(
        "SELECT COALESCE(sum(byte_size),0) FROM analyzer_cache",
        [],
        |row| row.get(0),
    )?;
    if cache_bytes > ANALYZER_CACHE_MAX_BYTES {
        report.issues.push(IntegrityIssue {
            code: "analyzer_cache_budget_exceeded".into(),
            path_or_id: cache_bytes.to_string(),
            guidance: "Evict oldest analyzer-cache rows; canonical AnalysisRuns and observations are unaffected.".into(),
        });
    }
    let mut statement = connection.prepare(
        "SELECT id FROM (
           SELECT ar.entity_id AS id FROM analysis_runs ar
             JOIN repository_baselines rb ON rb.entity_id=ar.baseline_id
             WHERE ar.project_id=?1 AND (rb.repository_id<>ar.repository_id OR rb.project_id<>ar.project_id)
           UNION
           SELECT o.id FROM code_entity_observations o
             JOIN analysis_runs ar ON ar.entity_id=o.analysis_run_id
             JOIN code_entities ce ON ce.entity_id=o.code_entity_id
             WHERE o.project_id=?1 AND (o.baseline_id<>ar.baseline_id OR ce.repository_id<>ar.repository_id
               OR ce.project_id<>o.project_id OR ar.project_id<>o.project_id)
           UNION
           SELECT o.id FROM test_observations o
             JOIN analysis_runs ar ON ar.entity_id=o.analysis_run_id
             JOIN tests t ON t.entity_id=o.test_id
             JOIN code_entities ce ON ce.entity_id=o.code_entity_id
             WHERE o.project_id=?1 AND (o.baseline_id<>ar.baseline_id OR t.repository_id<>ar.repository_id
               OR ce.repository_id<>ar.repository_id OR t.project_id<>o.project_id
               OR ce.project_id<>o.project_id OR ar.project_id<>o.project_id)
           UNION
           SELECT tr.entity_id FROM test_runs tr
             JOIN repository_baselines rb ON rb.entity_id=tr.baseline_id
             WHERE tr.project_id=?1 AND (rb.repository_id<>tr.repository_id OR rb.project_id<>tr.project_id)
           UNION
           SELECT tr.entity_id FROM test_runs tr
             JOIN test_run_results rr ON rr.test_run_id=tr.entity_id
             JOIN tests t ON t.entity_id=rr.test_id
             WHERE tr.project_id=?1 AND (t.repository_id<>tr.repository_id OR t.project_id<>tr.project_id)
         ) ORDER BY id",
    )?;
    let ids = statement
        .query_map([project_id], |row| row.get::<_, String>(0))?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    for id in ids {
        report.issues.push(IntegrityIssue {
            code: "invalid_code_intelligence_source_membership".into(),
            path_or_id: id,
            guidance: "Restore the affected CP5 observation from a verified backup or rebuild it from its exact same-Repository baseline.".into(),
        });
    }
    let mut statement = connection.prepare(
        "SELECT r.id FROM relationships r
         WHERE r.project_id=?1 AND (
           (r.source_entity_type='repository' AND r.relation_type='defines'
             AND ((r.target_entity_type='code_entity' AND EXISTS(
               SELECT 1 FROM code_entities ce WHERE ce.entity_id=r.target_entity_id
                 AND ce.repository_id<>r.source_entity_id))
              OR (r.target_entity_type='test' AND EXISTS(
               SELECT 1 FROM tests t WHERE t.entity_id=r.target_entity_id
                 AND t.repository_id<>r.source_entity_id))))
           OR (r.source_entity_type='change_set' AND r.relation_type='modifies'
             AND EXISTS(SELECT 1 FROM change_sets cs JOIN code_entities ce
               ON ce.entity_id=r.target_entity_id WHERE cs.entity_id=r.source_entity_id
                 AND cs.repository_id<>ce.repository_id))
           OR (r.source_entity_type='test' AND r.relation_type='verifies'
             AND r.target_entity_type='code_entity' AND EXISTS(
               SELECT 1 FROM tests t JOIN code_entities ce ON ce.entity_id=r.target_entity_id
               WHERE t.entity_id=r.source_entity_id AND t.repository_id<>ce.repository_id))
           OR (r.source_entity_type='test_run' AND r.relation_type='executes'
             AND EXISTS(SELECT 1 FROM test_runs tr JOIN tests t ON t.entity_id=r.target_entity_id
               WHERE tr.entity_id=r.source_entity_id AND tr.repository_id<>t.repository_id))
           OR (r.source_entity_type='test_run' AND r.relation_type='observed_at'
             AND EXISTS(SELECT 1 FROM test_runs tr JOIN repository_baselines rb
               ON rb.entity_id=r.target_entity_id WHERE tr.entity_id=r.source_entity_id
                 AND tr.repository_id<>rb.repository_id))
         ) ORDER BY r.id",
    )?;
    let ids = statement
        .query_map([project_id], |row| row.get::<_, String>(0))?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    for id in ids {
        report.issues.push(IntegrityIssue {
            code: "invalid_code_intelligence_relationship_membership".into(),
            path_or_id: id,
            guidance: "Quarantine the cross-Repository relationship and recreate it through the typed CP5 command after confirming its endpoints.".into(),
        });
    }
    let mut statement = connection.prepare(
        "SELECT e.id FROM entities e LEFT JOIN development_search_documents d ON d.entity_id=e.id
         WHERE e.project_id=?1 AND e.entity_type IN ('analysis_run','code_entity','test','test_run')
         AND d.entity_id IS NULL",
    )?;
    let ids = statement
        .query_map([project_id], |row| row.get::<_, String>(0))?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    for id in ids {
        report.issues.push(IntegrityIssue {
            code: "missing_code_intelligence_search_projection".into(),
            path_or_id: id,
            guidance:
                "Rebuild the deterministic Development search projection from canonical CP5 rows."
                    .into(),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_ast_grep_extracts_symbols_and_test_patterns() {
        let rust = analyze_text_file(
            "src/lib.rs",
            "#[test]\npub fn works() {}\nstruct Item;\nstruct Other;\n\
             impl Item { fn run(&self) {} }\nimpl Other { fn run(&self) {} }",
            Some("rust".into()),
        )
        .unwrap();
        assert!(rust.symbols.iter().any(|symbol| symbol.name == "works"));
        assert!(
            rust.symbols
                .iter()
                .any(|symbol| symbol.qualified_name == "Item::run")
        );
        assert!(
            rust.symbols
                .iter()
                .any(|symbol| symbol.qualified_name == "Other::run")
        );
        assert!(rust.tests.iter().any(|test| test.name == "works"));
        let js = analyze_text_file(
            "app.test.ts",
            "export function add(a:number,b:number){return a+b}\ntest('adds',()=>{});",
            Some("typescript".into()),
        )
        .unwrap();
        assert!(js.symbols.iter().any(|symbol| symbol.name == "add"));
        assert!(js.tests.iter().any(|test| test.name == "adds"));
    }

    #[test]
    fn manifests_extract_dependencies_without_dotenv_values() {
        let cargo = analyze_text_file("Cargo.toml", "[dependencies]\nserde = \"1\"", None).unwrap();
        assert!(
            cargo
                .dependencies
                .iter()
                .any(|dependency| dependency.name == "serde")
        );
        let env = analyze_configuration(".env", "TOKEN=secret\nPORT=3000").unwrap();
        assert!(env.sensitive_values_omitted);
        assert_eq!(env.top_level_keys, vec!["TOKEN", "PORT"]);
    }
}
