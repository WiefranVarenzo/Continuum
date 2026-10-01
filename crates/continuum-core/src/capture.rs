use std::collections::HashSet;
use std::io::Cursor;

use chrono::Utc;
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::store::{
    append_event_with_context, bounded_json, prior_result, record_command_with_context,
    validate_command_context, validate_nonempty,
};
use crate::{
    ArtifactClassification, CommandContext, ContinuityStore, CoreError, EvidenceKind,
    IntegrityIssue, IntegrityReport, NewEvidence, OriginKind, PageRequest, ResearchItem, Result,
    new_id,
};

const MAX_CAPTURE_JSON_BYTES: usize = 1024 * 1024;
const MAX_CAPTURE_SOURCES: usize = 3;
const MAX_CAPTURE_CHUNK_BYTES: usize = 32 * 1024 * 1024;
const MAX_CAPTURE_SEGMENT_BYTES: u64 = MAX_CAPTURE_CHUNK_BYTES as u64;
const MAX_CAPTURE_PAGE: u32 = 100;

macro_rules! string_enum {
    ($name:ident { $($variant:ident => $value:literal),+ $(,)? }) => {
        #[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
        #[serde(rename_all = "snake_case")]
        pub enum $name { $($variant),+ }

        impl $name {
            pub fn as_str(self) -> &'static str {
                match self { $(Self::$variant => $value),+ }
            }

            #[allow(dead_code)]
            fn parse(value: &str) -> Result<Self> {
                match value {
                    $($value => Ok(Self::$variant),)+
                    _ => Err(CoreError::Validation(format!(
                        "unsupported {} value {value}", stringify!($name)
                    ))),
                }
            }
        }
    };
}

string_enum!(CaptureSourceKind {
    Screen => "screen",
    SystemAudio => "system_audio",
    Microphone => "microphone",
});

string_enum!(CapturePermissionStatus {
    Unknown => "unknown",
    Granted => "granted",
    Denied => "denied",
    Unavailable => "unavailable",
    Revoked => "revoked",
});

string_enum!(CaptureSessionState {
    AwaitingPermission => "awaiting_permission",
    Ready => "ready",
    Blocked => "blocked",
    Capturing => "capturing",
    Paused => "paused",
    Finalizing => "finalizing",
    Completed => "completed",
    Interrupted => "interrupted",
    Failed => "failed",
    Cancelled => "cancelled",
});

string_enum!(CaptureExternalKind {
    Browser => "browser",
    File => "file",
    Web => "web",
    Screenshot => "screenshot",
});

string_enum!(CaptureDerivationKind {
    Transcription => "transcription",
    Ocr => "ocr",
    Thumbnail => "thumbnail",
    Waveform => "waveform",
});

string_enum!(CaptureBackpressureAction {
    Continue => "continue",
    FlushSegment => "flush_segment",
    ReduceQuality => "reduce_quality",
    Pause => "pause",
});

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CaptureBackendDescriptor {
    pub backend_id: String,
    pub backend_version: String,
    pub platform: String,
    pub capabilities: Vec<CaptureCapability>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CaptureCapability {
    pub source_kind: CaptureSourceKind,
    pub capability_id: String,
    pub available: bool,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CaptureSourceRequest {
    pub source_kind: CaptureSourceKind,
    pub capability_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CaptureEncodingSettings {
    pub container: String,
    pub video_codec: Option<String>,
    pub audio_codec: Option<String>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub frames_per_second: Option<u32>,
    pub sample_rate_hz: Option<u32>,
    pub channels: Option<u8>,
    pub segment_duration_ms: u64,
}

impl Default for CaptureEncodingSettings {
    fn default() -> Self {
        Self {
            container: "webm".into(),
            video_codec: Some("vp8".into()),
            audio_codec: Some("opus".into()),
            width: None,
            height: None,
            frames_per_second: Some(30),
            sample_rate_hz: Some(48_000),
            channels: Some(2),
            segment_duration_ms: 5_000,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CaptureBufferPolicy {
    pub max_buffer_bytes: u64,
    pub max_segment_bytes: u64,
    pub max_pending_segments: u32,
    pub quality_reduction_allowed: bool,
}

impl Default for CaptureBufferPolicy {
    fn default() -> Self {
        Self {
            max_buffer_bytes: 16 * 1024 * 1024,
            max_segment_bytes: 32 * 1024 * 1024,
            max_pending_segments: 4,
            quality_reduction_allowed: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CapturePressureSample {
    pub buffered_bytes: u64,
    pub pending_segments: u32,
}

pub fn evaluate_capture_pressure(
    policy: &CaptureBufferPolicy,
    sample: &CapturePressureSample,
) -> CaptureBackpressureAction {
    if sample.buffered_bytes >= policy.max_buffer_bytes
        || sample.pending_segments >= policy.max_pending_segments
    {
        return if policy.quality_reduction_allowed {
            CaptureBackpressureAction::ReduceQuality
        } else {
            CaptureBackpressureAction::Pause
        };
    }
    if sample.buffered_bytes.saturating_mul(100) >= policy.max_buffer_bytes.saturating_mul(75) {
        CaptureBackpressureAction::FlushSegment
    } else {
        CaptureBackpressureAction::Continue
    }
}

pub trait CaptureBackend: Send {
    fn descriptor(&self) -> CaptureBackendDescriptor;
    fn start(&mut self, session: &CaptureSession) -> Result<()>;
    fn pause(&mut self) -> Result<()>;
    fn resume(&mut self) -> Result<()>;
    fn stop(&mut self) -> Result<()>;
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NewCaptureSession {
    pub research_session_id: Option<String>,
    pub sources: Vec<CaptureSourceRequest>,
    pub backend: CaptureBackendDescriptor,
    pub encoding: CaptureEncodingSettings,
    pub buffer_policy: CaptureBufferPolicy,
    pub legal_consent_acknowledged: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CaptureSessionSource {
    pub source_kind: CaptureSourceKind,
    pub capability_id: String,
    pub permission_status: CapturePermissionStatus,
    pub permission_checked_at: Option<String>,
    pub permission_reference: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CaptureSession {
    pub id: String,
    pub project_id: String,
    pub research_session_id: Option<String>,
    pub state: CaptureSessionState,
    pub state_version: i64,
    pub backend: CaptureBackendDescriptor,
    pub encoding: CaptureEncodingSettings,
    pub buffer_policy: CaptureBufferPolicy,
    pub legal_consent_acknowledged: bool,
    pub indicator_id: Option<String>,
    pub recoverable: bool,
    pub failure_code: Option<String>,
    pub failure_message: Option<String>,
    pub started_at: Option<String>,
    pub paused_at: Option<String>,
    pub ended_at: Option<String>,
    pub created_at: String,
    pub created_by: String,
    pub updated_at: String,
    pub updated_by: String,
    pub sources: Vec<CaptureSessionSource>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CaptureSessionPage {
    pub items: Vec<CaptureSession>,
    pub next_offset: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CaptureSegment {
    pub id: String,
    pub project_id: String,
    pub capture_session_id: String,
    pub sequence: u32,
    pub state: String,
    pub artifact_id: String,
    pub media_type: String,
    pub source_kinds: Vec<CaptureSourceKind>,
    pub start_offset_ms: u64,
    pub end_offset_ms: u64,
    pub byte_size: u64,
    pub sha256: String,
    pub recovery_note: Option<String>,
    pub created_at: String,
    pub created_by: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NewCaptureSegment {
    pub sequence: u32,
    pub media_type: String,
    pub source_kinds: Vec<CaptureSourceKind>,
    pub start_offset_ms: u64,
    pub end_offset_ms: u64,
    pub complete: bool,
    pub recovery_note: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CaptureMarker {
    pub id: String,
    pub project_id: String,
    pub capture_session_id: String,
    pub segment_id: Option<String>,
    pub offset_ms: u64,
    pub label: String,
    pub note: String,
    pub created_at: String,
    pub created_by: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NewCaptureMarker {
    pub segment_id: Option<String>,
    pub offset_ms: u64,
    pub label: String,
    pub note: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NewCaptureSegmentEvidence {
    pub segment_id: String,
    pub start_offset_ms: u64,
    pub end_offset_ms: u64,
    pub title: String,
    pub annotation: String,
    pub summary: String,
    pub relevance: String,
    pub question_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CaptureExternalEvidence {
    pub kind: CaptureExternalKind,
    pub title: String,
    pub source_uri: Option<String>,
    pub source_title: Option<String>,
    pub captured_at: Option<String>,
    pub media_type: Option<String>,
    pub bytes: Option<Vec<u8>>,
    pub source_content: Option<String>,
    pub annotation: String,
    pub summary: String,
    pub relevance: String,
    pub research_session_id: Option<String>,
    pub research_question_id: Option<String>,
    pub capture_session_id: Option<String>,
    pub classification: ArtifactClassification,
    #[serde(default)]
    pub metadata: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NewCaptureDerivation {
    pub source_artifact_id: String,
    pub kind: CaptureDerivationKind,
    pub engine_id: String,
    pub engine_version: String,
    pub settings: Value,
    pub media_type: String,
    pub bytes: Vec<u8>,
    pub classification: ArtifactClassification,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CaptureDerivation {
    pub id: String,
    pub source_artifact_id: String,
    pub derived_artifact_id: String,
    pub kind: CaptureDerivationKind,
    pub engine_id: String,
    pub engine_version: String,
    pub created_at: String,
    pub created_by: String,
}

impl ContinuityStore {
    pub fn create_capture_session(
        &self,
        command: &CommandContext,
        input: NewCaptureSession,
    ) -> Result<CaptureSession> {
        validate_capture_session_input(&input)?;
        validate_command_context(command)?;
        let mut connection = self.connection()?;
        let tx = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        require_research_enabled(&tx, &self.manifest.project_id)?;
        if let Some(Some(id)) = prior_result(
            &tx,
            &self.manifest.project_id,
            command,
            "CreateCaptureSession",
        )? {
            tx.commit()?;
            let existing = self.get_capture_session(&id)?;
            if !capture_session_matches(&existing, &input) {
                return Err(CoreError::Conflict(
                    "capture session idempotency key was reused with different configuration"
                        .into(),
                ));
            }
            return Ok(existing);
        }
        if let Some(research_session_id) = &input.research_session_id {
            require_active_research_session(&tx, &self.manifest.project_id, research_session_id)?;
        }
        for source in &input.sources {
            let capability = input
                .backend
                .capabilities
                .iter()
                .find(|capability| {
                    capability.source_kind == source.source_kind
                        && capability.capability_id == source.capability_id
                })
                .ok_or_else(|| {
                    CoreError::Validation(format!(
                        "backend does not declare capability {}",
                        source.capability_id
                    ))
                })?;
            if !capability.available {
                return Err(CoreError::Validation(format!(
                    "capture capability {} is unavailable: {}",
                    capability.capability_id,
                    capability.reason.as_deref().unwrap_or("no reason supplied")
                )));
            }
        }
        let id = new_id();
        let now = Utc::now().to_rfc3339();
        let capability_snapshot = bounded_json(
            &serde_json::to_value(&input.backend.capabilities)?,
            MAX_CAPTURE_JSON_BYTES,
            "capture capability snapshot",
        )?;
        let encoding = bounded_json(
            &serde_json::to_value(&input.encoding)?,
            MAX_CAPTURE_JSON_BYTES,
            "capture encoding",
        )?;
        let buffer_policy = bounded_json(
            &serde_json::to_value(&input.buffer_policy)?,
            MAX_CAPTURE_JSON_BYTES,
            "capture buffer policy",
        )?;
        tx.execute(
            "INSERT INTO capture_sessions(
                id,project_id,research_session_entity_id,state,state_version,backend_id,
                backend_version,backend_platform,capability_snapshot_json,encoding_settings_json,buffer_policy_json,
                legal_consent_acknowledged,created_at,created_by,updated_at,updated_by
             ) VALUES(?1,?2,?3,'awaiting_permission',1,?4,?5,?6,?7,?8,?9,?10,?11,?12,?11,?12)",
            params![
                id,
                self.manifest.project_id,
                input.research_session_id,
                input.backend.backend_id,
                input.backend.backend_version,
                input.backend.platform,
                capability_snapshot,
                encoding,
                buffer_policy,
                input.legal_consent_acknowledged,
                now,
                command.actor.id
            ],
        )?;
        for source in &input.sources {
            tx.execute(
                "INSERT INTO capture_session_sources(capture_session_id,source_kind,capability_id,permission_status)
                 VALUES(?1,?2,?3,'unknown')",
                params![id, source.source_kind.as_str(), source.capability_id],
            )?;
        }
        append_event_with_context(
            &tx,
            &self.manifest.project_id,
            command,
            Some(&id),
            "capture.session.created",
            &json!({"capture_session_id":id,"source_count":input.sources.len(),"backend_id":input.backend.backend_id}),
        )?;
        record_command_with_context(
            &tx,
            command,
            &self.manifest.project_id,
            "CreateCaptureSession",
            Some(&id),
            None,
            &json!({"capture_session_id":id,"sources":input.sources}),
        )?;
        tx.commit()?;
        self.get_capture_session(&id)
    }

    pub fn record_capture_permission(
        &self,
        command: &CommandContext,
        capture_session_id: &str,
        source_kind: CaptureSourceKind,
        status: CapturePermissionStatus,
        permission_reference: Option<&str>,
        expected_version: i64,
    ) -> Result<CaptureSession> {
        validate_command_context(command)?;
        if status == CapturePermissionStatus::Unknown {
            return Err(CoreError::Validation(
                "a permission observation cannot return to unknown".into(),
            ));
        }
        if permission_reference.is_some_and(|value| value.chars().count() > 500) {
            return Err(CoreError::Validation(
                "permission reference exceeds 500 characters".into(),
            ));
        }
        let mut connection = self.connection()?;
        let tx = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        if prior_result(
            &tx,
            &self.manifest.project_id,
            command,
            "RecordCapturePermission",
        )?
        .is_some()
        {
            tx.commit()?;
            return self.get_capture_session(capture_session_id);
        }
        let (state, version): (String, i64) = tx
            .query_row(
                "SELECT state,state_version FROM capture_sessions WHERE id=?1 AND project_id=?2",
                params![capture_session_id, self.manifest.project_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?
            .ok_or_else(|| CoreError::NotFound(capture_session_id.into()))?;
        if version != expected_version {
            return Err(CoreError::Conflict(
                "capture session version is stale".into(),
            ));
        }
        if matches!(state.as_str(), "completed" | "failed" | "cancelled") {
            return Err(CoreError::Conflict(
                "terminal capture session permissions cannot change".into(),
            ));
        }
        if matches!(state.as_str(), "capturing" | "paused" | "finalizing")
            && status != CapturePermissionStatus::Revoked
        {
            return Err(CoreError::Conflict(
                "only an OS permission revocation may change permissions during capture".into(),
            ));
        }
        let now = Utc::now().to_rfc3339();
        let changed = tx.execute(
            "UPDATE capture_session_sources
             SET permission_status=?3,permission_checked_at=?4,permission_reference=?5
             WHERE capture_session_id=?1 AND source_kind=?2",
            params![
                capture_session_id,
                source_kind.as_str(),
                status.as_str(),
                now,
                permission_reference
            ],
        )?;
        if changed != 1 {
            return Err(CoreError::Validation(
                "permission source was not requested by this capture session".into(),
            ));
        }
        tx.execute(
            "INSERT INTO capture_permission_events(
                id,project_id,capture_session_id,source_kind,permission_status,
                permission_reference,occurred_at,actor_id
             ) VALUES(?1,?2,?3,?4,?5,?6,?7,?8)",
            params![
                new_id(),
                self.manifest.project_id,
                capture_session_id,
                source_kind.as_str(),
                status.as_str(),
                permission_reference,
                now,
                command.actor.id
            ],
        )?;
        let next_state = permission_aggregate_state(&tx, capture_session_id)?;
        let revoked_while_active = status == CapturePermissionStatus::Revoked
            && matches!(state.as_str(), "capturing" | "paused" | "finalizing");
        let effective_state = if revoked_while_active {
            CaptureSessionState::Interrupted
        } else {
            next_state
        };
        tx.execute(
            "UPDATE capture_sessions SET state=?3,state_version=state_version+1,
                indicator_id=CASE WHEN ?3='interrupted' THEN NULL ELSE indicator_id END,
                recoverable=CASE WHEN ?3='interrupted' THEN 1 ELSE recoverable END,
                ended_at=CASE WHEN ?3='interrupted' THEN ?4 ELSE ended_at END,
                updated_at=?4,updated_by=?5
             WHERE id=?1 AND project_id=?2 AND state_version=?6",
            params![
                capture_session_id,
                self.manifest.project_id,
                effective_state.as_str(),
                now,
                command.actor.id,
                expected_version
            ],
        )?;
        append_event_with_context(
            &tx,
            &self.manifest.project_id,
            command,
            Some(capture_session_id),
            "capture.permission.recorded",
            &json!({"capture_session_id":capture_session_id,"source_kind":source_kind,"status":status,"session_state":effective_state}),
        )?;
        record_command_with_context(
            &tx,
            command,
            &self.manifest.project_id,
            "RecordCapturePermission",
            Some(capture_session_id),
            Some(expected_version),
            &json!({"source_kind":source_kind,"status":status}),
        )?;
        tx.commit()?;
        self.get_capture_session(capture_session_id)
    }

    pub fn begin_capture(
        &self,
        command: &CommandContext,
        capture_session_id: &str,
        indicator_id: &str,
        expected_version: i64,
    ) -> Result<CaptureSession> {
        validate_nonempty(indicator_id, 200, "capture indicator id")?;
        transition_capture_session(
            self,
            command,
            capture_session_id,
            expected_version,
            "BeginCapture",
            &[CaptureSessionState::Ready],
            CaptureSessionState::Capturing,
            Some(indicator_id),
            None,
            None,
            false,
        )
    }

    pub fn pause_capture(
        &self,
        command: &CommandContext,
        capture_session_id: &str,
        expected_version: i64,
    ) -> Result<CaptureSession> {
        let indicator = self
            .get_capture_session(capture_session_id)?
            .indicator_id
            .ok_or_else(|| CoreError::Conflict("capture indicator is not active".into()))?;
        transition_capture_session(
            self,
            command,
            capture_session_id,
            expected_version,
            "PauseCapture",
            &[CaptureSessionState::Capturing],
            CaptureSessionState::Paused,
            Some(&indicator),
            None,
            None,
            false,
        )
    }

    pub fn resume_capture(
        &self,
        command: &CommandContext,
        capture_session_id: &str,
        indicator_id: &str,
        expected_version: i64,
    ) -> Result<CaptureSession> {
        validate_nonempty(indicator_id, 200, "capture indicator id")?;
        transition_capture_session(
            self,
            command,
            capture_session_id,
            expected_version,
            "ResumeCapture",
            &[CaptureSessionState::Paused],
            CaptureSessionState::Capturing,
            Some(indicator_id),
            None,
            None,
            false,
        )
    }

    pub fn stop_capture(
        &self,
        command: &CommandContext,
        capture_session_id: &str,
        expected_version: i64,
    ) -> Result<CaptureSession> {
        transition_capture_session(
            self,
            command,
            capture_session_id,
            expected_version,
            "StopCapture",
            &[CaptureSessionState::Capturing, CaptureSessionState::Paused],
            CaptureSessionState::Completed,
            None,
            None,
            None,
            false,
        )
    }

    pub fn interrupt_capture(
        &self,
        command: &CommandContext,
        capture_session_id: &str,
        reason: &str,
        expected_version: i64,
    ) -> Result<CaptureSession> {
        validate_nonempty(reason, 1_000, "capture interruption reason")?;
        transition_capture_session(
            self,
            command,
            capture_session_id,
            expected_version,
            "InterruptCapture",
            &[CaptureSessionState::Capturing, CaptureSessionState::Paused],
            CaptureSessionState::Interrupted,
            None,
            Some("interrupted"),
            Some(reason),
            true,
        )
    }

    pub fn fail_capture(
        &self,
        command: &CommandContext,
        capture_session_id: &str,
        failure_code: &str,
        failure_message: &str,
        recoverable: bool,
        expected_version: i64,
    ) -> Result<CaptureSession> {
        validate_nonempty(failure_code, 100, "capture failure code")?;
        validate_nonempty(failure_message, 1_000, "capture failure message")?;
        transition_capture_session(
            self,
            command,
            capture_session_id,
            expected_version,
            "FailCapture",
            &[
                CaptureSessionState::AwaitingPermission,
                CaptureSessionState::Ready,
                CaptureSessionState::Blocked,
                CaptureSessionState::Capturing,
                CaptureSessionState::Paused,
                CaptureSessionState::Finalizing,
                CaptureSessionState::Interrupted,
            ],
            CaptureSessionState::Failed,
            None,
            Some(failure_code),
            Some(failure_message),
            recoverable,
        )
    }

    pub fn cancel_capture(
        &self,
        command: &CommandContext,
        capture_session_id: &str,
        expected_version: i64,
    ) -> Result<CaptureSession> {
        transition_capture_session(
            self,
            command,
            capture_session_id,
            expected_version,
            "CancelCapture",
            &[
                CaptureSessionState::AwaitingPermission,
                CaptureSessionState::Ready,
                CaptureSessionState::Blocked,
            ],
            CaptureSessionState::Cancelled,
            None,
            None,
            None,
            false,
        )
    }

    pub fn ingest_capture_segment(
        &self,
        command: &CommandContext,
        capture_session_id: &str,
        input: NewCaptureSegment,
        bytes: &[u8],
        classification: ArtifactClassification,
    ) -> Result<CaptureSegment> {
        validate_command_context(command)?;
        validate_capture_segment_input(&input, bytes)?;
        let input_sha256 = hex::encode(Sha256::digest(bytes));
        if let Some(id) =
            prior_capture_result(self, command, "IngestCaptureSegment", capture_session_id)?
        {
            let existing = self.get_capture_segment(&id)?;
            let expected_state = if input.complete {
                "finalized"
            } else {
                "recoverable"
            };
            if existing.sequence != input.sequence
                || existing.sha256 != input_sha256
                || existing.media_type != input.media_type
                || existing.source_kinds != input.source_kinds
                || existing.start_offset_ms != input.start_offset_ms
                || existing.end_offset_ms != input.end_offset_ms
                || existing.state != expected_state
            {
                return Err(CoreError::Conflict(
                    "capture segment idempotency key was reused with different content or metadata"
                        .into(),
                ));
            }
            return Ok(existing);
        }
        let session = self.get_capture_session(capture_session_id)?;
        if !matches!(
            session.state,
            CaptureSessionState::Capturing
                | CaptureSessionState::Paused
                | CaptureSessionState::Interrupted
        ) {
            return Err(CoreError::Conflict(
                "capture segments require an active, paused, or interrupted session".into(),
            ));
        }
        if bytes.len() as u64 > session.buffer_policy.max_segment_bytes {
            return Err(CoreError::Validation(
                "capture segment exceeds the session buffer policy".into(),
            ));
        }
        let container = input
            .media_type
            .split(';')
            .next()
            .and_then(|value| value.rsplit('/').next())
            .unwrap_or("")
            .trim();
        if !container.eq_ignore_ascii_case(&session.encoding.container) {
            return Err(CoreError::Validation(
                "capture segment media type does not match the session container".into(),
            ));
        }
        let requested = session
            .sources
            .iter()
            .map(|source| source.source_kind)
            .collect::<HashSet<_>>();
        if input
            .source_kinds
            .iter()
            .any(|source| !requested.contains(source))
        {
            return Err(CoreError::Validation(
                "capture segment contains a source not requested by the session".into(),
            ));
        }
        let artifact_command = child_command(command, "capture-segment-artifact");
        let artifact = self.ingest_artifact_reader_with_context(
            &artifact_command,
            Cursor::new(bytes),
            &input.media_type,
            classification,
            OriginKind::Deterministic,
            &json!({
                "capture_session_id":capture_session_id,
                "sequence":input.sequence,
                "complete":input.complete,
                "source_kinds":input.source_kinds
            }),
        )?;
        let mut connection = self.connection()?;
        let tx = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        if let Some(Some(id)) = prior_result(
            &tx,
            &self.manifest.project_id,
            command,
            "IngestCaptureSegment",
        )? {
            tx.commit()?;
            return self.get_capture_segment(&id);
        }
        let next_sequence: i64 = tx.query_row(
            "SELECT COALESCE(max(sequence)+1,0) FROM capture_segments WHERE capture_session_id=?1",
            [capture_session_id],
            |row| row.get(0),
        )?;
        if i64::from(input.sequence) != next_sequence {
            return Err(CoreError::Conflict(format!(
                "capture segment sequence must be {next_sequence}"
            )));
        }
        let previous_end: i64 = tx.query_row(
            "SELECT COALESCE(max(end_offset_ms),0) FROM capture_segments WHERE capture_session_id=?1",
            [capture_session_id],
            |row| row.get(0),
        )?;
        if input.start_offset_ms < previous_end as u64 {
            return Err(CoreError::Conflict(
                "capture segment time range overlaps a prior segment".into(),
            ));
        }
        let id = new_id();
        let now = Utc::now().to_rfc3339();
        let source_kinds_json = bounded_json(
            &serde_json::to_value(&input.source_kinds)?,
            MAX_CAPTURE_JSON_BYTES,
            "capture segment sources",
        )?;
        let state = if input.complete {
            "finalized"
        } else {
            "recoverable"
        };
        let start_offset = capture_i64(input.start_offset_ms, "capture segment start")?;
        let end_offset = capture_i64(input.end_offset_ms, "capture segment end")?;
        tx.execute(
            "INSERT INTO capture_segments(
                id,project_id,capture_session_id,sequence,state,artifact_id,media_type,
                source_kinds_json,start_offset_ms,end_offset_ms,byte_size,sha256,
                recovery_note,created_at,created_by
             ) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15)",
            params![
                id,
                self.manifest.project_id,
                capture_session_id,
                input.sequence,
                state,
                artifact.id,
                input.media_type,
                source_kinds_json,
                start_offset,
                end_offset,
                artifact.byte_size,
                artifact.sha256,
                input.recovery_note,
                now,
                command.actor.id
            ],
        )?;
        if !input.complete {
            tx.execute(
                "UPDATE capture_sessions SET recoverable=1,updated_at=?2,updated_by=?3
                 WHERE id=?1 AND project_id=?4",
                params![
                    capture_session_id,
                    now,
                    command.actor.id,
                    self.manifest.project_id
                ],
            )?;
        }
        append_event_with_context(
            &tx,
            &self.manifest.project_id,
            command,
            Some(capture_session_id),
            "capture.segment.finalized",
            &json!({"capture_session_id":capture_session_id,"segment_id":id,"artifact_id":artifact.id,"sequence":input.sequence,"state":state}),
        )?;
        record_command_with_context(
            &tx,
            command,
            &self.manifest.project_id,
            "IngestCaptureSegment",
            Some(&id),
            None,
            &json!({"capture_session_id":capture_session_id,"artifact_id":artifact.id,"sequence":input.sequence,"sha256":artifact.sha256}),
        )?;
        tx.commit()?;
        self.get_capture_segment(&id)
    }

    pub fn add_capture_marker(
        &self,
        command: &CommandContext,
        capture_session_id: &str,
        input: NewCaptureMarker,
    ) -> Result<CaptureMarker> {
        validate_command_context(command)?;
        validate_nonempty(&input.label, 500, "capture marker label")?;
        if input.note.chars().count() > 10_000 {
            return Err(CoreError::Validation(
                "capture marker note exceeds 10000 characters".into(),
            ));
        }
        let session = self.get_capture_session(capture_session_id)?;
        if !matches!(
            session.state,
            CaptureSessionState::Capturing | CaptureSessionState::Paused
        ) {
            return Err(CoreError::Conflict(
                "markers can be created only while capture is active or paused".into(),
            ));
        }
        let mut connection = self.connection()?;
        let tx = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        if let Some(Some(id)) =
            prior_result(&tx, &self.manifest.project_id, command, "AddCaptureMarker")?
        {
            tx.commit()?;
            return self.get_capture_marker(&id);
        }
        if let Some(segment_id) = &input.segment_id {
            let range: Option<(i64, i64)> = tx
                .query_row(
                    "SELECT start_offset_ms,end_offset_ms FROM capture_segments
                     WHERE id=?1 AND capture_session_id=?2 AND project_id=?3",
                    params![segment_id, capture_session_id, self.manifest.project_id],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .optional()?;
            let Some((start, end)) = range else {
                return Err(CoreError::Validation(
                    "capture marker segment is missing or belongs to another session".into(),
                ));
            };
            if input.offset_ms < start as u64 || input.offset_ms > end as u64 {
                return Err(CoreError::Validation(
                    "capture marker offset is outside its segment".into(),
                ));
            }
        }
        let id = new_id();
        let now = Utc::now().to_rfc3339();
        let offset = capture_i64(input.offset_ms, "capture marker offset")?;
        tx.execute(
            "INSERT INTO capture_markers(
                id,project_id,capture_session_id,segment_id,offset_ms,label,note,created_at,created_by
             ) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9)",
            params![
                id,
                self.manifest.project_id,
                capture_session_id,
                input.segment_id,
                offset,
                input.label,
                input.note,
                now,
                command.actor.id
            ],
        )?;
        append_event_with_context(
            &tx,
            &self.manifest.project_id,
            command,
            Some(capture_session_id),
            "capture.marker.created",
            &json!({"capture_session_id":capture_session_id,"marker_id":id,"offset_ms":input.offset_ms}),
        )?;
        record_command_with_context(
            &tx,
            command,
            &self.manifest.project_id,
            "AddCaptureMarker",
            Some(&id),
            None,
            &json!({"capture_session_id":capture_session_id,"offset_ms":input.offset_ms}),
        )?;
        tx.commit()?;
        self.get_capture_marker(&id)
    }

    /// MediaRecorder timeslices form one stream, not independently playable files.
    /// Assemble in order, preserving the originals and their provenance.
    pub fn promote_capture_recording_to_evidence(
        &self, command: &CommandContext, capture_session_id: &str, title: &str, annotation: &str,
    ) -> Result<ResearchItem> {
        validate_command_context(command)?;
        validate_nonempty(title, 1_000, "recording title")?;
        if annotation.len() > 10_000 { return Err(CoreError::Validation("Recording note is too long".into())); }
        let session = self.get_capture_session(capture_session_id)?;
        if session.state != CaptureSessionState::Completed {
            return Err(CoreError::Validation("Finish the recording before adding it to the workspace".into()));
        }
        let reference=format!("continuum:capture-session:{capture_session_id}");
        let existing:Option<String>=self.connection()?.query_row(
            "SELECT e.entity_id FROM evidence e JOIN entities n ON n.id=e.entity_id WHERE n.project_id=?1 AND e.stable_reference=?2 AND e.capture_method='capture.complete_recording' LIMIT 1",
            params![self.manifest.project_id,reference],|row|row.get(0)).optional()?;
        if let Some(id)=existing {return self.get_research_item(&id);}
        let mut segments = Vec::new();
        loop {
            let page = self.list_capture_segments(capture_session_id, PageRequest {limit:100,offset:segments.len() as u64})?;
            if page.is_empty() { break; }
            segments.extend(page);
            if segments.len()>10_000 {return Err(CoreError::Validation("Recording has too many segments to assemble".into()));}
        }
        let Some(first) = segments.first() else {return Err(CoreError::Validation("This recording contains no saved media".into()));};
        if session.backend.backend_id != "webview-media-recorder" || !["video/webm","audio/webm"].iter().any(|mime|first.media_type.starts_with(mime)) {
            return Err(CoreError::Validation("This recording format cannot be assembled here".into()));
        }
        let mut bytes=Vec::new();
        let mut end=0;
        for (index,segment) in segments.iter().enumerate() {
            if segment.sequence as usize!=index || segment.start_offset_ms!=end || segment.media_type!=first.media_type || segment.state!="finalized" {
                return Err(CoreError::Validation("Recording segments are incomplete or inconsistent; original segments are preserved".into()));
            }
            if bytes.len() as u64 + segment.byte_size > 64*1024*1024 {
                return Err(CoreError::Validation("This recording exceeds the 64 MiB workspace playback limit. Its original segments are safely stored; use shorter recordings for now".into()));
            }
            if !matches!(self.get_artifact(&segment.artifact_id)?.classification.as_str(),"public"|"internal") {
                return Err(CoreError::Validation("A recording source has restricted privacy. Assembly will not lower its protection".into()));
            }
            bytes.extend(self.read_artifact_bounded(&segment.artifact_id, MAX_CAPTURE_CHUNK_BYTES as u64)?);
            end=segment.end_offset_ms;
        }
        let artifact=self.ingest_artifact_reader_with_context(&child_command(command,"recording-artifact"),Cursor::new(bytes),&first.media_type,
            ArtifactClassification::Internal,OriginKind::Deterministic,&json!({"capture_session_id":capture_session_id,"assembly":"ordered-media-recorder-timeslices"}))?;
        self.create_evidence(&child_command(command,"recording-evidence"),NewEvidence {
            title:title.into(),kind:EvidenceKind::RecordingSegment,origin:OriginKind::Deterministic,
            source_uri:None,source_title:Some(title.into()),source_author:None,captured_at:Some(first.created_at.clone()),
            capture_method:"capture.complete_recording".into(),stable_reference:Some(reference),
            source_content:None,annotation:annotation.into(),summary:if annotation.trim().is_empty(){"Recording saved; AI interpretation has not been requested.".into()}else{annotation.into()},
            relevance:"Original recording from this research session; interpretation requires review.".into(),original_artifact_id:Some(artifact.id),
            question_id:None,session_id:session.research_session_id,
            metadata:json!({"capture_session_id":capture_session_id,"capture_segment_ids":segments.iter().map(|s|s.id.clone()).collect::<Vec<_>>(),"start_offset_ms":0,"end_offset_ms":end,"assembled_recording":true,"media_type":first.media_type}),
        })
    }

    pub fn promote_capture_segment_to_evidence(
        &self,
        command: &CommandContext,
        input: NewCaptureSegmentEvidence,
    ) -> Result<ResearchItem> {
        validate_command_context(command)?;
        validate_nonempty(&input.title, 1_000, "Evidence title")?;
        let segment = self.get_capture_segment(&input.segment_id)?;
        if input.start_offset_ms < segment.start_offset_ms
            || input.end_offset_ms > segment.end_offset_ms
            || input.end_offset_ms < input.start_offset_ms
        {
            return Err(CoreError::Validation(
                "Evidence range must be inside the capture segment".into(),
            ));
        }
        if let Some(evidence_id) = prior_capture_result(
            self,
            command,
            "PromoteCaptureSegmentToEvidence",
            &segment.capture_session_id,
        )? {
            return self.get_research_item(&evidence_id);
        }
        let session = self.get_capture_session(&segment.capture_session_id)?;
        let evidence_command = child_command(command, "segment-evidence");
        let evidence = self.create_evidence(
            &evidence_command,
            NewEvidence {
                title: input.title.clone(),
                kind: EvidenceKind::RecordingSegment,
                origin: OriginKind::Deterministic,
                source_uri: None,
                source_title: Some(input.title),
                source_author: None,
                captured_at: Some(segment.created_at.clone()),
                capture_method: "capture.recording_segment".into(),
                stable_reference: Some(format!(
                    "continuum:capture-segment:{}#{}-{}",
                    input.segment_id, input.start_offset_ms, input.end_offset_ms
                )),
                source_content: None,
                annotation: input.annotation,
                summary: input.summary,
                relevance: input.relevance,
                original_artifact_id: Some(segment.artifact_id.clone()),
                question_id: input.question_id,
                session_id: session.research_session_id,
                metadata: json!({
                    "capture_session_id":segment.capture_session_id,
                    "capture_segment_id":segment.id,
                    "start_offset_ms":input.start_offset_ms,
                    "end_offset_ms":input.end_offset_ms,
                    "partial_source":segment.state=="recoverable"
                }),
            },
        )?;
        self.record_capture_evidence_link(
            command,
            &segment.capture_session_id,
            &evidence.entity.id,
            "segment",
            Some(&input.segment_id),
            None,
            Some(input.start_offset_ms),
            Some(input.end_offset_ms),
            "PromoteCaptureSegmentToEvidence",
        )?;
        Ok(evidence)
    }

    pub fn promote_capture_marker_to_evidence(
        &self,
        command: &CommandContext,
        marker_id: &str,
        title: &str,
        summary: &str,
        relevance: &str,
        question_id: Option<String>,
    ) -> Result<ResearchItem> {
        validate_command_context(command)?;
        validate_nonempty(title, 1_000, "Evidence title")?;
        let marker = self.get_capture_marker(marker_id)?;
        if let Some(evidence_id) = prior_capture_result(
            self,
            command,
            "PromoteCaptureMarkerToEvidence",
            &marker.capture_session_id,
        )? {
            return self.get_research_item(&evidence_id);
        }
        let session = self.get_capture_session(&marker.capture_session_id)?;
        let evidence_command = child_command(command, "marker-evidence");
        let evidence = self.create_evidence(
            &evidence_command,
            NewEvidence {
                title: title.into(),
                kind: EvidenceKind::CaptureMarker,
                origin: OriginKind::User,
                source_uri: None,
                source_title: Some(marker.label.clone()),
                source_author: None,
                captured_at: Some(marker.created_at.clone()),
                capture_method: "capture.marker".into(),
                stable_reference: Some(format!("continuum:capture-marker:{marker_id}")),
                source_content: Some(marker.note.clone()),
                annotation: marker.note,
                summary: summary.into(),
                relevance: relevance.into(),
                original_artifact_id: None,
                question_id,
                session_id: session.research_session_id,
                metadata: json!({
                    "capture_session_id":marker.capture_session_id,
                    "capture_marker_id":marker.id,
                    "offset_ms":marker.offset_ms
                }),
            },
        )?;
        self.record_capture_evidence_link(
            command,
            &marker.capture_session_id,
            &evidence.entity.id,
            "marker",
            None,
            Some(marker_id),
            Some(marker.offset_ms),
            Some(marker.offset_ms),
            "PromoteCaptureMarkerToEvidence",
        )?;
        Ok(evidence)
    }

    pub fn capture_external_evidence(
        &self,
        command: &CommandContext,
        input: CaptureExternalEvidence,
    ) -> Result<ResearchItem> {
        validate_external_capture(&input)?;
        let request_fingerprint = external_capture_fingerprint(&input)?;
        if let Some(evidence_id) = prior_capture_result(
            self,
            command,
            "CaptureExternalEvidence",
            input.capture_session_id.as_deref().unwrap_or("external"),
        )? {
            let stored: Option<String> = self
                .connection()?
                .query_row(
                    "SELECT request_fingerprint FROM capture_external_items
                     WHERE evidence_entity_id=?1 AND project_id=?2",
                    params![evidence_id, self.manifest.project_id],
                    |row| row.get(0),
                )
                .optional()?;
            if stored.as_deref() != Some(request_fingerprint.as_str()) {
                return Err(CoreError::Conflict(
                    "external capture idempotency key was reused with different content or metadata"
                        .into(),
                ));
            }
            return self.get_research_item(&evidence_id);
        }
        let artifact = if let Some(bytes) = &input.bytes {
            let artifact_command = child_command(command, "external-capture-artifact");
            Some(self.ingest_artifact_reader_with_context(
                &artifact_command,
                Cursor::new(bytes),
                input.media_type.as_deref().expect("validated media type"),
                input.classification,
                OriginKind::Import,
                &json!({"capture_kind":input.kind,"source_uri":&input.source_uri}),
            )?)
        } else {
            None
        };
        if let Some(capture_session_id) = &input.capture_session_id {
            self.get_capture_session(capture_session_id)?;
        }
        let evidence_command = child_command(command, "external-capture-evidence");
        let evidence_kind = match input.kind {
            CaptureExternalKind::Browser | CaptureExternalKind::Web => EvidenceKind::Web,
            CaptureExternalKind::File => EvidenceKind::File,
            CaptureExternalKind::Screenshot => EvidenceKind::Screenshot,
        };
        let stable_reference = input.source_uri.clone().or_else(|| {
            artifact
                .as_ref()
                .map(|artifact| format!("sha256:{}", artifact.sha256))
        });
        let evidence = self.create_evidence(
            &evidence_command,
            NewEvidence {
                title: input.title.clone(),
                kind: evidence_kind,
                origin: OriginKind::Import,
                source_uri: input.source_uri.clone(),
                source_title: input.source_title,
                source_author: None,
                captured_at: input.captured_at.clone(),
                capture_method: format!("capture.{}", input.kind.as_str()),
                stable_reference,
                source_content: input.source_content,
                annotation: input.annotation,
                summary: input.summary,
                relevance: input.relevance,
                original_artifact_id: artifact.as_ref().map(|artifact| artifact.id.clone()),
                question_id: input.research_question_id,
                session_id: input.research_session_id,
                metadata: json!({
                    "continuum_capture": {
                        "kind": input.kind,
                        "capture_session_id": input.capture_session_id
                    },
                    "source_metadata": input.metadata
                }),
            },
        )?;
        let mut connection = self.connection()?;
        let tx = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        if prior_result(
            &tx,
            &self.manifest.project_id,
            command,
            "CaptureExternalEvidence",
        )?
        .is_none()
        {
            tx.execute(
                "INSERT INTO capture_external_items(
                    id,project_id,capture_kind,evidence_entity_id,artifact_id,source_uri,
                    request_fingerprint,captured_at,created_by
                 ) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9)",
                params![
                    new_id(),
                    self.manifest.project_id,
                    input.kind.as_str(),
                    evidence.entity.id,
                    artifact.as_ref().map(|artifact| &artifact.id),
                    input.source_uri,
                    request_fingerprint,
                    input.captured_at.unwrap_or_else(|| Utc::now().to_rfc3339()),
                    command.actor.id
                ],
            )?;
            append_event_with_context(
                &tx,
                &self.manifest.project_id,
                command,
                Some(&evidence.entity.id),
                "capture.external.evidence_created",
                &json!({"evidence_id":evidence.entity.id,"capture_kind":input.kind,"artifact_id":artifact.as_ref().map(|artifact|&artifact.id)}),
            )?;
            record_command_with_context(
                &tx,
                command,
                &self.manifest.project_id,
                "CaptureExternalEvidence",
                Some(&evidence.entity.id),
                None,
                &json!({"evidence_id":evidence.entity.id,"capture_kind":input.kind}),
            )?;
        }
        tx.commit()?;
        if let Some(capture_session_id) = &input.capture_session_id {
            let link_command = child_command(command, "external-capture-link");
            self.record_capture_evidence_link(
                &link_command,
                capture_session_id,
                &evidence.entity.id,
                "external",
                None,
                None,
                None,
                None,
                "LinkExternalCaptureEvidence",
            )?;
        }
        Ok(evidence)
    }

    pub fn create_capture_derivation(
        &self,
        command: &CommandContext,
        input: NewCaptureDerivation,
    ) -> Result<CaptureDerivation> {
        validate_command_context(command)?;
        validate_nonempty(&input.engine_id, 200, "derivation engine id")?;
        validate_nonempty(&input.engine_version, 100, "derivation engine version")?;
        validate_nonempty(&input.media_type, 200, "derived media type")?;
        if input.bytes.is_empty() || input.bytes.len() > MAX_CAPTURE_CHUNK_BYTES {
            return Err(CoreError::Validation(
                "derived capture payload must fit the bounded chunk contract".into(),
            ));
        }
        bounded_json(
            &input.settings,
            MAX_CAPTURE_JSON_BYTES,
            "derivation settings",
        )?;
        if let Some(id) = prior_capture_result(
            self,
            command,
            "CreateCaptureDerivation",
            &input.source_artifact_id,
        )? {
            return self.get_capture_derivation(&id);
        }
        let source_artifact = self.get_artifact(&input.source_artifact_id)?;
        if hex::encode(Sha256::digest(&input.bytes)) == source_artifact.sha256 {
            return Err(CoreError::Validation(
                "a derived capture artifact cannot replace or equal its source".into(),
            ));
        }
        let artifact_command = child_command(command, "capture-derived-artifact");
        let derived = self.ingest_artifact_reader_with_context(
            &artifact_command,
            Cursor::new(&input.bytes),
            &input.media_type,
            input.classification,
            OriginKind::Deterministic,
            &json!({"derived_from":input.source_artifact_id,"derivation_kind":input.kind}),
        )?;
        if derived.id == input.source_artifact_id {
            return Err(CoreError::Validation(
                "a derived capture artifact cannot replace or equal its source".into(),
            ));
        }
        let mut connection = self.connection()?;
        let tx = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        if let Some(Some(id)) = prior_result(
            &tx,
            &self.manifest.project_id,
            command,
            "CreateCaptureDerivation",
        )? {
            tx.commit()?;
            return self.get_capture_derivation(&id);
        }
        let id = new_id();
        let now = Utc::now().to_rfc3339();
        tx.execute(
            "INSERT INTO capture_derivations(
                id,project_id,source_artifact_id,derived_artifact_id,derivation_kind,
                engine_id,engine_version,settings_json,created_at,created_by
             ) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)",
            params![
                id,
                self.manifest.project_id,
                input.source_artifact_id,
                derived.id,
                input.kind.as_str(),
                input.engine_id,
                input.engine_version,
                serde_json::to_string(&input.settings)?,
                now,
                command.actor.id
            ],
        )?;
        append_event_with_context(
            &tx,
            &self.manifest.project_id,
            command,
            Some(&input.source_artifact_id),
            "capture.derivation.created",
            &json!({"derivation_id":id,"source_artifact_id":input.source_artifact_id,"derived_artifact_id":derived.id,"kind":input.kind}),
        )?;
        record_command_with_context(
            &tx,
            command,
            &self.manifest.project_id,
            "CreateCaptureDerivation",
            Some(&id),
            None,
            &json!({"source_artifact_id":input.source_artifact_id,"derived_artifact_id":derived.id,"kind":input.kind}),
        )?;
        tx.commit()?;
        self.get_capture_derivation(&id)
    }

    pub fn get_capture_session(&self, id: &str) -> Result<CaptureSession> {
        read_capture_session(&self.connection()?, &self.manifest.project_id, id)
    }

    pub fn list_capture_sessions(&self, page: PageRequest) -> Result<CaptureSessionPage> {
        validate_capture_page(page)?;
        let offset = i64::try_from(page.offset)
            .map_err(|_| CoreError::Validation("capture page offset is too large".into()))?;
        let connection = self.connection()?;
        let mut statement = connection.prepare(
            "SELECT id FROM capture_sessions WHERE project_id=?1
             ORDER BY created_at DESC,id DESC LIMIT ?2 OFFSET ?3",
        )?;
        let ids = statement
            .query_map(
                params![self.manifest.project_id, page.limit + 1, offset],
                |row| row.get::<_, String>(0),
            )?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        let has_more = ids.len() > page.limit as usize;
        let items = ids
            .into_iter()
            .take(page.limit as usize)
            .map(|id| read_capture_session(&connection, &self.manifest.project_id, &id))
            .collect::<Result<Vec<_>>>()?;
        Ok(CaptureSessionPage {
            next_offset: has_more.then_some(page.offset + items.len() as u64),
            items,
        })
    }

    pub fn get_capture_segment(&self, id: &str) -> Result<CaptureSegment> {
        read_capture_segment(&self.connection()?, &self.manifest.project_id, id)
    }

    pub fn list_capture_segments(
        &self,
        capture_session_id: &str,
        page: PageRequest,
    ) -> Result<Vec<CaptureSegment>> {
        validate_capture_page(page)?;
        self.get_capture_session(capture_session_id)?;
        let offset = i64::try_from(page.offset)
            .map_err(|_| CoreError::Validation("capture page offset is too large".into()))?;
        let connection = self.connection()?;
        let mut statement = connection.prepare(
            "SELECT id FROM capture_segments WHERE project_id=?1 AND capture_session_id=?2
             ORDER BY sequence,id LIMIT ?3 OFFSET ?4",
        )?;
        statement
            .query_map(
                params![
                    self.manifest.project_id,
                    capture_session_id,
                    page.limit,
                    offset
                ],
                |row| row.get::<_, String>(0),
            )?
            .collect::<std::result::Result<Vec<_>, _>>()?
            .into_iter()
            .map(|id| read_capture_segment(&connection, &self.manifest.project_id, &id))
            .collect()
    }

    pub fn get_capture_marker(&self, id: &str) -> Result<CaptureMarker> {
        read_capture_marker(&self.connection()?, &self.manifest.project_id, id)
    }

    pub fn list_capture_markers(
        &self,
        capture_session_id: &str,
        page: PageRequest,
    ) -> Result<Vec<CaptureMarker>> {
        validate_capture_page(page)?;
        self.get_capture_session(capture_session_id)?;
        let offset = i64::try_from(page.offset)
            .map_err(|_| CoreError::Validation("capture page offset is too large".into()))?;
        let connection = self.connection()?;
        let mut statement = connection.prepare(
            "SELECT id FROM capture_markers WHERE project_id=?1 AND capture_session_id=?2
             ORDER BY offset_ms,id LIMIT ?3 OFFSET ?4",
        )?;
        statement
            .query_map(
                params![
                    self.manifest.project_id,
                    capture_session_id,
                    page.limit,
                    offset
                ],
                |row| row.get::<_, String>(0),
            )?
            .collect::<std::result::Result<Vec<_>, _>>()?
            .into_iter()
            .map(|id| read_capture_marker(&connection, &self.manifest.project_id, &id))
            .collect()
    }

    pub fn get_capture_derivation(&self, id: &str) -> Result<CaptureDerivation> {
        self.connection()?
            .query_row(
                "SELECT id,source_artifact_id,derived_artifact_id,derivation_kind,
                        engine_id,engine_version,created_at,created_by
                 FROM capture_derivations WHERE id=?1 AND project_id=?2",
                params![id, self.manifest.project_id],
                |row| {
                    let kind: String = row.get(3)?;
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        kind,
                        row.get::<_, String>(4)?,
                        row.get::<_, String>(5)?,
                        row.get::<_, String>(6)?,
                        row.get::<_, String>(7)?,
                    ))
                },
            )
            .optional()?
            .ok_or_else(|| CoreError::NotFound(id.into()))
            .and_then(|row| {
                Ok(CaptureDerivation {
                    id: row.0,
                    source_artifact_id: row.1,
                    derived_artifact_id: row.2,
                    kind: CaptureDerivationKind::parse(&row.3)?,
                    engine_id: row.4,
                    engine_version: row.5,
                    created_at: row.6,
                    created_by: row.7,
                })
            })
    }

    #[allow(clippy::too_many_arguments)]
    fn record_capture_evidence_link(
        &self,
        command: &CommandContext,
        capture_session_id: &str,
        evidence_id: &str,
        source_kind: &str,
        segment_id: Option<&str>,
        marker_id: Option<&str>,
        start_offset_ms: Option<u64>,
        end_offset_ms: Option<u64>,
        operation: &str,
    ) -> Result<()> {
        let mut connection = self.connection()?;
        let tx = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        if prior_result(&tx, &self.manifest.project_id, command, operation)?.is_some() {
            return tx.commit().map_err(Into::into);
        }
        let start_offset = start_offset_ms
            .map(|value| capture_i64(value, "Evidence start offset"))
            .transpose()?;
        let end_offset = end_offset_ms
            .map(|value| capture_i64(value, "Evidence end offset"))
            .transpose()?;
        tx.execute(
            "INSERT INTO capture_evidence_links(
                id,project_id,capture_session_id,evidence_entity_id,source_kind,
                segment_id,marker_id,start_offset_ms,end_offset_ms,created_at,created_by
             ) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)",
            params![
                new_id(),
                self.manifest.project_id,
                capture_session_id,
                evidence_id,
                source_kind,
                segment_id,
                marker_id,
                start_offset,
                end_offset,
                Utc::now().to_rfc3339(),
                command.actor.id
            ],
        )?;
        append_event_with_context(
            &tx,
            &self.manifest.project_id,
            command,
            Some(evidence_id),
            "capture.evidence.linked",
            &json!({"capture_session_id":capture_session_id,"evidence_id":evidence_id,"source_kind":source_kind,"segment_id":segment_id,"marker_id":marker_id}),
        )?;
        record_command_with_context(
            &tx,
            command,
            &self.manifest.project_id,
            operation,
            Some(evidence_id),
            None,
            &json!({"capture_session_id":capture_session_id,"source_kind":source_kind}),
        )?;
        tx.commit()?;
        Ok(())
    }
}

#[allow(clippy::too_many_arguments)]
fn transition_capture_session(
    store: &ContinuityStore,
    command: &CommandContext,
    id: &str,
    expected_version: i64,
    operation: &str,
    allowed: &[CaptureSessionState],
    next: CaptureSessionState,
    indicator_id: Option<&str>,
    failure_code: Option<&str>,
    reason: Option<&str>,
    recoverable: bool,
) -> Result<CaptureSession> {
    validate_command_context(command)?;
    let mut connection = store.connection()?;
    let tx = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    if prior_result(&tx, &store.manifest.project_id, command, operation)?.is_some() {
        tx.commit()?;
        return store.get_capture_session(id);
    }
    let current: (String, i64, bool) = tx
        .query_row(
            "SELECT state,state_version,legal_consent_acknowledged FROM capture_sessions
             WHERE id=?1 AND project_id=?2",
            params![id, store.manifest.project_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()?
        .ok_or_else(|| CoreError::NotFound(id.into()))?;
    if current.1 != expected_version {
        return Err(CoreError::Conflict(
            "capture session version is stale".into(),
        ));
    }
    let current_state = CaptureSessionState::parse(&current.0)?;
    if !allowed.contains(&current_state) {
        return Err(CoreError::Conflict(format!(
            "capture transition {operation} is invalid from {}",
            current_state.as_str()
        )));
    }
    if next == CaptureSessionState::Capturing && !current.2 {
        return Err(CoreError::Validation(
            "lawful-capture responsibility must be acknowledged before capture".into(),
        ));
    }
    let now = Utc::now().to_rfc3339();
    let is_terminal = matches!(
        next,
        CaptureSessionState::Completed
            | CaptureSessionState::Interrupted
            | CaptureSessionState::Failed
            | CaptureSessionState::Cancelled
    );
    let changed = tx.execute(
        "UPDATE capture_sessions SET state=?3,state_version=state_version+1,
            indicator_id=?4,recoverable=?5,
            failure_code=COALESCE(?6,failure_code),
            failure_message=COALESCE(?7,failure_message),
            started_at=CASE WHEN ?3='capturing' AND started_at IS NULL THEN ?8 ELSE started_at END,
            paused_at=CASE WHEN ?3='paused' THEN ?8 WHEN ?3='capturing' THEN NULL ELSE paused_at END,
            ended_at=CASE WHEN ?9 THEN ?8 ELSE ended_at END,
            updated_at=?8,updated_by=?10
         WHERE id=?1 AND project_id=?2 AND state_version=?11",
        params![
            id,
            store.manifest.project_id,
            next.as_str(),
            indicator_id,
            recoverable,
            failure_code,
            reason,
            now,
            is_terminal,
            command.actor.id,
            expected_version
        ],
    )?;
    if changed != 1 {
        return Err(CoreError::Conflict(
            "capture session transition lost a race".into(),
        ));
    }
    append_event_with_context(
        &tx,
        &store.manifest.project_id,
        command,
        Some(id),
        &format!("capture.session.{}", next.as_str()),
        &json!({"capture_session_id":id,"from":current_state,"to":next,"recoverable":recoverable,"failure_code":failure_code}),
    )?;
    record_command_with_context(
        &tx,
        command,
        &store.manifest.project_id,
        operation,
        Some(id),
        Some(expected_version),
        &json!({"capture_session_id":id,"from":current_state,"to":next}),
    )?;
    tx.commit()?;
    store.get_capture_session(id)
}

fn child_command(parent: &CommandContext, purpose: &str) -> CommandContext {
    let mut command = CommandContext::new(parent.actor.clone());
    command.idempotency_key = format!("{}:{purpose}", parent.idempotency_key);
    command.correlation_id = parent.correlation_id.clone();
    command.causation_id = Some(parent.command_id.clone());
    command
}

fn prior_capture_result(
    store: &ContinuityStore,
    command: &CommandContext,
    operation: &str,
    _scope: &str,
) -> Result<Option<String>> {
    validate_command_context(command)?;
    let mut connection = store.connection()?;
    let tx = connection.transaction_with_behavior(TransactionBehavior::Deferred)?;
    let result = prior_result(&tx, &store.manifest.project_id, command, operation)?.flatten();
    tx.commit()?;
    Ok(result)
}

fn capture_i64(value: u64, label: &str) -> Result<i64> {
    i64::try_from(value).map_err(|_| CoreError::Validation(format!("{label} is too large")))
}

fn validate_capture_session_input(input: &NewCaptureSession) -> Result<()> {
    if input.sources.is_empty() || input.sources.len() > MAX_CAPTURE_SOURCES {
        return Err(CoreError::Validation(
            "capture session must request 1..=3 sources".into(),
        ));
    }
    validate_nonempty(&input.backend.backend_id, 200, "capture backend id")?;
    validate_nonempty(
        &input.backend.backend_version,
        100,
        "capture backend version",
    )?;
    validate_nonempty(&input.backend.platform, 100, "capture backend platform")?;
    validate_nonempty(&input.encoding.container, 50, "capture container")?;
    let unique = input
        .sources
        .iter()
        .map(|source| source.source_kind)
        .collect::<HashSet<_>>();
    if unique.len() != input.sources.len() {
        return Err(CoreError::Validation(
            "capture source kinds must be unique".into(),
        ));
    }
    let mut capability_keys = HashSet::new();
    for capability in &input.backend.capabilities {
        validate_nonempty(&capability.capability_id, 200, "capture capability id")?;
        if capability
            .reason
            .as_ref()
            .is_some_and(|value| value.chars().count() > 1_000)
        {
            return Err(CoreError::Validation(
                "capture capability reason exceeds 1000 characters".into(),
            ));
        }
        if !capability_keys.insert((capability.source_kind, capability.capability_id.as_str())) {
            return Err(CoreError::Validation(
                "capture backend capability entries must be unique".into(),
            ));
        }
    }
    for source in &input.sources {
        validate_nonempty(&source.capability_id, 200, "capture capability id")?;
    }
    if input.encoding.segment_duration_ms < 1_000 || input.encoding.segment_duration_ms > 60_000 {
        return Err(CoreError::Validation(
            "capture segment duration must be within 1..=60 seconds".into(),
        ));
    }
    if input
        .encoding
        .frames_per_second
        .is_some_and(|value| value == 0 || value > 120)
        || input
            .encoding
            .width
            .is_some_and(|value| value == 0 || value > 16_384)
        || input
            .encoding
            .height
            .is_some_and(|value| value == 0 || value > 16_384)
        || input
            .encoding
            .channels
            .is_some_and(|value| value == 0 || value > 8)
        || input
            .encoding
            .sample_rate_hz
            .is_some_and(|value| !(8_000..=384_000).contains(&value))
    {
        return Err(CoreError::Validation(
            "capture encoding setting exceeds its safety bounds".into(),
        ));
    }
    if input.buffer_policy.max_buffer_bytes == 0
        || input.buffer_policy.max_segment_bytes == 0
        || input.buffer_policy.max_segment_bytes > MAX_CAPTURE_SEGMENT_BYTES
        || input.buffer_policy.max_buffer_bytes > input.buffer_policy.max_segment_bytes
        || input.buffer_policy.max_pending_segments == 0
        || input.buffer_policy.max_pending_segments > 64
    {
        return Err(CoreError::Validation(
            "capture buffer policy exceeds its safety bounds".into(),
        ));
    }
    bounded_json(
        &serde_json::to_value(input)?,
        MAX_CAPTURE_JSON_BYTES,
        "capture session request",
    )?;
    Ok(())
}

fn capture_session_matches(existing: &CaptureSession, input: &NewCaptureSession) -> bool {
    let existing_sources = existing
        .sources
        .iter()
        .map(|source| (source.source_kind, source.capability_id.as_str()))
        .collect::<HashSet<_>>();
    let requested_sources = input
        .sources
        .iter()
        .map(|source| (source.source_kind, source.capability_id.as_str()))
        .collect::<HashSet<_>>();
    existing.research_session_id == input.research_session_id
        && existing.backend == input.backend
        && existing.encoding == input.encoding
        && existing.buffer_policy == input.buffer_policy
        && existing.legal_consent_acknowledged == input.legal_consent_acknowledged
        && existing_sources == requested_sources
}

fn external_capture_fingerprint(input: &CaptureExternalEvidence) -> Result<String> {
    let bytes_sha256 = input
        .bytes
        .as_ref()
        .map(|bytes| hex::encode(Sha256::digest(bytes)));
    let material = bounded_json(
        &json!({
            "kind":input.kind,
            "title":input.title,
            "source_uri":input.source_uri,
            "source_title":input.source_title,
            "captured_at":input.captured_at,
            "media_type":input.media_type,
            "bytes_sha256":bytes_sha256,
            "source_content":input.source_content,
            "annotation":input.annotation,
            "summary":input.summary,
            "relevance":input.relevance,
            "research_session_id":input.research_session_id,
            "research_question_id":input.research_question_id,
            "capture_session_id":input.capture_session_id,
            "classification":input.classification,
            "metadata":input.metadata
        }),
        MAX_CAPTURE_JSON_BYTES,
        "external capture fingerprint material",
    )?;
    Ok(hex::encode(Sha256::digest(material.as_bytes())))
}

fn validate_capture_segment_input(input: &NewCaptureSegment, bytes: &[u8]) -> Result<()> {
    validate_nonempty(&input.media_type, 200, "capture media type")?;
    if bytes.is_empty() || bytes.len() > MAX_CAPTURE_CHUNK_BYTES {
        return Err(CoreError::Validation(format!(
            "capture segment bytes must be within 1..={MAX_CAPTURE_CHUNK_BYTES}"
        )));
    }
    if input.source_kinds.is_empty() || input.source_kinds.len() > MAX_CAPTURE_SOURCES {
        return Err(CoreError::Validation(
            "capture segment must identify 1..=3 sources".into(),
        ));
    }
    if input
        .source_kinds
        .iter()
        .copied()
        .collect::<HashSet<_>>()
        .len()
        != input.source_kinds.len()
    {
        return Err(CoreError::Validation(
            "capture segment source kinds must be unique".into(),
        ));
    }
    if input.end_offset_ms <= input.start_offset_ms {
        return Err(CoreError::Validation(
            "capture segment end must be later than its start".into(),
        ));
    }
    if input
        .recovery_note
        .as_ref()
        .is_some_and(|value| value.chars().count() > 2_000)
    {
        return Err(CoreError::Validation(
            "capture recovery note exceeds 2000 characters".into(),
        ));
    }
    Ok(())
}

fn validate_external_capture(input: &CaptureExternalEvidence) -> Result<()> {
    validate_nonempty(&input.title, 1_000, "captured Evidence title")?;
    if matches!(
        input.kind,
        CaptureExternalKind::File | CaptureExternalKind::Screenshot
    ) && input.bytes.as_ref().is_none_or(Vec::is_empty)
    {
        return Err(CoreError::Validation(
            "file and screenshot capture require original bytes".into(),
        ));
    }
    if matches!(
        input.kind,
        CaptureExternalKind::Browser | CaptureExternalKind::Web
    ) && input
        .source_uri
        .as_deref()
        .is_none_or(|value| value.trim().is_empty())
        && input
            .source_content
            .as_deref()
            .is_none_or(|value| value.trim().is_empty())
    {
        return Err(CoreError::Validation(
            "browser/web capture requires a source URI or captured content".into(),
        ));
    }
    if let Some(bytes) = &input.bytes {
        if bytes.is_empty() || bytes.len() > MAX_CAPTURE_CHUNK_BYTES {
            return Err(CoreError::Validation(
                "external capture payload exceeds the bounded chunk contract".into(),
            ));
        }
        validate_nonempty(
            input.media_type.as_deref().unwrap_or(""),
            200,
            "captured media type",
        )?;
    }
    if input
        .source_uri
        .as_ref()
        .is_some_and(|value| value.chars().count() > 4_096)
        || input
            .source_content
            .as_ref()
            .is_some_and(|value| value.chars().count() > 1_000_000)
    {
        return Err(CoreError::Validation(
            "external capture text exceeds its bound".into(),
        ));
    }
    bounded_json(&input.metadata, MAX_CAPTURE_JSON_BYTES, "capture metadata")?;
    Ok(())
}

fn validate_capture_page(page: PageRequest) -> Result<()> {
    if page.limit == 0 || page.limit > MAX_CAPTURE_PAGE {
        Err(CoreError::Validation(
            "capture page limit must be within 1..=100".into(),
        ))
    } else {
        Ok(())
    }
}

fn require_research_enabled(connection: &Connection, project_id: &str) -> Result<()> {
    let state: Option<(String, bool)> = connection
        .query_row(
            "SELECT p.status,c.enabled FROM projects p JOIN space_capabilities c ON c.project_id=p.id
             WHERE p.id=?1 AND c.space='research'",
            [project_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?;
    match state {
        Some((status, true)) if status == "active" => Ok(()),
        Some((status, _)) if status != "active" => {
            Err(CoreError::Conflict("project is archived".into()))
        }
        _ => Err(CoreError::Conflict(
            "Research Space is disabled for this project".into(),
        )),
    }
}

fn require_active_research_session(
    connection: &Connection,
    project_id: &str,
    id: &str,
) -> Result<()> {
    let state: Option<String> = connection
        .query_row(
            "SELECT e.status FROM entities e JOIN research_sessions r ON r.entity_id=e.id
             WHERE e.id=?1 AND e.project_id=?2",
            params![id, project_id],
            |row| row.get(0),
        )
        .optional()?;
    match state.as_deref() {
        Some("active") => Ok(()),
        Some(_) => Err(CoreError::Conflict("research session is not active".into())),
        None => Err(CoreError::Validation(
            "research session is missing or belongs to another project".into(),
        )),
    }
}

fn permission_aggregate_state(
    connection: &Connection,
    capture_session_id: &str,
) -> Result<CaptureSessionState> {
    let counts: (i64, i64, i64) = connection.query_row(
        "SELECT count(*),
                sum(CASE WHEN permission_status='granted' THEN 1 ELSE 0 END),
                sum(CASE WHEN permission_status IN ('denied','unavailable','revoked') THEN 1 ELSE 0 END)
         FROM capture_session_sources WHERE capture_session_id=?1",
        [capture_session_id],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    )?;
    if counts.2 > 0 {
        Ok(CaptureSessionState::Blocked)
    } else if counts.0 > 0 && counts.0 == counts.1 {
        Ok(CaptureSessionState::Ready)
    } else {
        Ok(CaptureSessionState::AwaitingPermission)
    }
}

fn read_capture_session(
    connection: &Connection,
    project_id: &str,
    id: &str,
) -> Result<CaptureSession> {
    type SessionRow = (
        String,
        String,
        Option<String>,
        String,
        i64,
        String,
        String,
        String,
        String,
        String,
        String,
        bool,
        Option<String>,
        bool,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
        String,
        String,
        String,
        String,
    );
    let row: SessionRow = connection
        .query_row(
            "SELECT id,project_id,research_session_entity_id,state,state_version,backend_id,
                    backend_version,backend_platform,capability_snapshot_json,encoding_settings_json,buffer_policy_json,
                    legal_consent_acknowledged,indicator_id,recoverable,failure_code,failure_message,
                    started_at,paused_at,ended_at,created_at,created_by,updated_at,updated_by
             FROM capture_sessions WHERE id=?1 AND project_id=?2",
            params![id, project_id],
            |row| {
                Ok((
                    row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?,
                    row.get(6)?, row.get(7)?, row.get(8)?, row.get(9)?, row.get(10)?, row.get(11)?,
                    row.get(12)?, row.get(13)?, row.get(14)?, row.get(15)?, row.get(16)?, row.get(17)?,
                    row.get(18)?, row.get(19)?, row.get(20)?, row.get(21)?, row.get(22)?,
                ))
            },
        )
        .optional()?
        .ok_or_else(|| CoreError::NotFound(id.into()))?;
    let capabilities: Vec<CaptureCapability> = serde_json::from_str(&row.8)?;
    let mut statement = connection.prepare(
        "SELECT source_kind,capability_id,permission_status,permission_checked_at,permission_reference
         FROM capture_session_sources WHERE capture_session_id=?1 ORDER BY source_kind",
    )?;
    let sources = statement
        .query_map([id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, Option<String>>(3)?,
                row.get::<_, Option<String>>(4)?,
            ))
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?
        .into_iter()
        .map(|source| {
            Ok(CaptureSessionSource {
                source_kind: CaptureSourceKind::parse(&source.0)?,
                capability_id: source.1,
                permission_status: CapturePermissionStatus::parse(&source.2)?,
                permission_checked_at: source.3,
                permission_reference: source.4,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(CaptureSession {
        id: row.0,
        project_id: row.1,
        research_session_id: row.2,
        state: CaptureSessionState::parse(&row.3)?,
        state_version: row.4,
        backend: CaptureBackendDescriptor {
            backend_id: row.5,
            backend_version: row.6,
            platform: row.7,
            capabilities,
        },
        encoding: serde_json::from_str(&row.9)?,
        buffer_policy: serde_json::from_str(&row.10)?,
        legal_consent_acknowledged: row.11,
        indicator_id: row.12,
        recoverable: row.13,
        failure_code: row.14,
        failure_message: row.15,
        started_at: row.16,
        paused_at: row.17,
        ended_at: row.18,
        created_at: row.19,
        created_by: row.20,
        updated_at: row.21,
        updated_by: row.22,
        sources,
    })
}

fn read_capture_segment(
    connection: &Connection,
    project_id: &str,
    id: &str,
) -> Result<CaptureSegment> {
    let row = connection
        .query_row(
            "SELECT id,project_id,capture_session_id,sequence,state,artifact_id,media_type,
                    source_kinds_json,start_offset_ms,end_offset_ms,byte_size,sha256,
                    recovery_note,created_at,created_by
             FROM capture_segments WHERE id=?1 AND project_id=?2",
            params![id, project_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, String>(6)?,
                    row.get::<_, String>(7)?,
                    row.get::<_, i64>(8)?,
                    row.get::<_, i64>(9)?,
                    row.get::<_, i64>(10)?,
                    row.get::<_, String>(11)?,
                    row.get::<_, Option<String>>(12)?,
                    row.get::<_, String>(13)?,
                    row.get::<_, String>(14)?,
                ))
            },
        )
        .optional()?
        .ok_or_else(|| CoreError::NotFound(id.into()))?;
    Ok(CaptureSegment {
        id: row.0,
        project_id: row.1,
        capture_session_id: row.2,
        sequence: u32::try_from(row.3)
            .map_err(|_| CoreError::Validation("invalid capture segment sequence".into()))?,
        state: row.4,
        artifact_id: row.5,
        media_type: row.6,
        source_kinds: serde_json::from_str(&row.7)?,
        start_offset_ms: u64::try_from(row.8)
            .map_err(|_| CoreError::Validation("invalid capture start offset".into()))?,
        end_offset_ms: u64::try_from(row.9)
            .map_err(|_| CoreError::Validation("invalid capture end offset".into()))?,
        byte_size: u64::try_from(row.10)
            .map_err(|_| CoreError::Validation("invalid capture byte size".into()))?,
        sha256: row.11,
        recovery_note: row.12,
        created_at: row.13,
        created_by: row.14,
    })
}

fn read_capture_marker(
    connection: &Connection,
    project_id: &str,
    id: &str,
) -> Result<CaptureMarker> {
    connection
        .query_row(
            "SELECT id,project_id,capture_session_id,segment_id,offset_ms,label,note,created_at,created_by
             FROM capture_markers WHERE id=?1 AND project_id=?2",
            params![id, project_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?,
                    row.get::<_, Option<String>>(3)?, row.get::<_, i64>(4)?, row.get::<_, String>(5)?,
                    row.get::<_, String>(6)?, row.get::<_, String>(7)?, row.get::<_, String>(8)?,
                ))
            },
        )
        .optional()?
        .ok_or_else(|| CoreError::NotFound(id.into()))
        .and_then(|row| {
            Ok(CaptureMarker {
                id: row.0,
                project_id: row.1,
                capture_session_id: row.2,
                segment_id: row.3,
                offset_ms: u64::try_from(row.4)
                    .map_err(|_| CoreError::Validation("invalid capture marker offset".into()))?,
                label: row.5,
                note: row.6,
                created_at: row.7,
                created_by: row.8,
            })
        })
}

pub(crate) fn append_capture_integrity_issues(
    connection: &Connection,
    project_id: &str,
    report: &mut IntegrityReport,
) -> Result<()> {
    let checks = [
        (
            "capture_source_missing",
            "SELECT id FROM capture_sessions s WHERE s.project_id=?1 AND NOT EXISTS(
                SELECT 1 FROM capture_session_sources x WHERE x.capture_session_id=s.id)",
            "Restore the capture source rows from backup or quarantine the session metadata.",
        ),
        (
            "capture_segment_artifact_mismatch",
            "SELECT s.id FROM capture_segments s JOIN artifacts a ON a.id=s.artifact_id
             WHERE s.project_id=?1 AND (a.project_id<>s.project_id OR a.sha256<>s.sha256 OR a.byte_size<>s.byte_size)",
            "Quarantine the segment and recover its exact content-addressed Artifact metadata.",
        ),
        (
            "capture_segment_evidence_mismatch",
            "SELECT l.id FROM capture_evidence_links l JOIN capture_segments s ON s.id=l.segment_id
             JOIN evidence e ON e.entity_id=l.evidence_entity_id
             WHERE l.project_id=?1 AND l.source_kind='segment' AND e.original_artifact_id<>s.artifact_id",
            "Repair the Evidence link from verified capture provenance; never duplicate or substitute media bytes.",
        ),
        (
            "capture_external_evidence_mismatch",
            "SELECT x.id FROM capture_external_items x JOIN evidence e ON e.entity_id=x.evidence_entity_id
             JOIN entities n ON n.id=e.entity_id
             WHERE x.project_id=?1 AND (
                n.project_id<>x.project_id OR x.artifact_id IS NOT e.original_artifact_id OR
                (x.capture_kind IN ('file','screenshot') AND x.artifact_id IS NULL) OR
                (x.capture_kind='file' AND e.evidence_kind<>'file') OR
                (x.capture_kind='screenshot' AND e.evidence_kind<>'screenshot') OR
                (x.capture_kind IN ('browser','web') AND e.evidence_kind<>'web')
             )",
            "Restore the external capture record and Evidence from the same verified source Artifact.",
        ),
        (
            "capture_session_scope_mismatch",
            "SELECT s.id FROM capture_sessions s JOIN research_sessions r ON r.entity_id=s.research_session_entity_id
             JOIN entities e ON e.id=r.entity_id WHERE s.project_id=?1 AND e.project_id<>s.project_id",
            "Detach the invalid cross-project session reference and restore it from project-scoped audit evidence.",
        ),
        (
            "capture_child_scope_mismatch",
            "SELECT x.id FROM (
                SELECT g.id,g.project_id,g.capture_session_id FROM capture_segments g
                UNION ALL SELECT m.id,m.project_id,m.capture_session_id FROM capture_markers m
             ) x JOIN capture_sessions s ON s.id=x.capture_session_id
             WHERE x.project_id=?1 AND s.project_id<>x.project_id",
            "Quarantine the child capture record; capture sessions and children must remain in one project.",
        ),
        (
            "capture_marker_segment_mismatch",
            "SELECT m.id FROM capture_markers m JOIN capture_segments s ON s.id=m.segment_id
             WHERE m.project_id=?1 AND (s.project_id<>m.project_id OR s.capture_session_id<>m.capture_session_id OR
                m.offset_ms<s.start_offset_ms OR m.offset_ms>s.end_offset_ms)",
            "Restore the marker to its original session/segment range from append-only capture events.",
        ),
        (
            "capture_evidence_scope_or_range_mismatch",
            "SELECT l.id FROM capture_evidence_links l
             JOIN capture_sessions c ON c.id=l.capture_session_id
             JOIN entities e ON e.id=l.evidence_entity_id
             LEFT JOIN capture_segments s ON s.id=l.segment_id
             LEFT JOIN capture_markers m ON m.id=l.marker_id
             WHERE l.project_id=?1 AND (
                c.project_id<>l.project_id OR e.project_id<>l.project_id OR
                (l.source_kind='segment' AND (s.project_id<>l.project_id OR s.capture_session_id<>l.capture_session_id OR
                    l.start_offset_ms<s.start_offset_ms OR l.end_offset_ms>s.end_offset_ms)) OR
                (l.source_kind='marker' AND (m.project_id<>l.project_id OR m.capture_session_id<>l.capture_session_id OR
                    l.start_offset_ms<>m.offset_ms OR l.end_offset_ms<>m.offset_ms)) OR
                (l.source_kind='external' AND NOT EXISTS(
                    SELECT 1 FROM capture_external_items x WHERE x.evidence_entity_id=l.evidence_entity_id AND x.project_id=l.project_id))
             )",
            "Rebuild the Evidence link from its project-scoped segment, marker, or external capture record.",
        ),
        (
            "capture_evidence_link_missing",
            "SELECT e.id FROM entities e WHERE e.project_id=?1 AND e.entity_type='evidence' AND (
                (json_extract(e.metadata_json,'$.capture_segment_id') IS NOT NULL AND NOT EXISTS(
                    SELECT 1 FROM capture_evidence_links l WHERE l.evidence_entity_id=e.id AND l.source_kind='segment')) OR
                (json_extract(e.metadata_json,'$.capture_marker_id') IS NOT NULL AND NOT EXISTS(
                    SELECT 1 FROM capture_evidence_links l WHERE l.evidence_entity_id=e.id AND l.source_kind='marker')) OR
                (json_extract(e.metadata_json,'$.continuum_capture.kind') IS NOT NULL AND NOT EXISTS(
                    SELECT 1 FROM capture_external_items x WHERE x.evidence_entity_id=e.id))
             )",
            "Retry the idempotent capture command to complete its Evidence metadata/link publication.",
        ),
        (
            "capture_derivation_scope_mismatch",
            "SELECT d.id FROM capture_derivations d JOIN artifacts s ON s.id=d.source_artifact_id
             JOIN artifacts x ON x.id=d.derived_artifact_id
             WHERE d.project_id=?1 AND (s.project_id<>d.project_id OR x.project_id<>d.project_id)",
            "Quarantine the derivation metadata and restore project-local source/derived Artifact references.",
        ),
    ];
    for (code, sql, guidance) in checks {
        let mut statement = connection.prepare(sql)?;
        let ids = statement
            .query_map([project_id], |row| row.get::<_, String>(0))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        report
            .issues
            .extend(ids.into_iter().map(|id| IntegrityIssue {
                code: code.into(),
                path_or_id: id,
                guidance: guidance.into(),
            }));
    }
    Ok(())
}
