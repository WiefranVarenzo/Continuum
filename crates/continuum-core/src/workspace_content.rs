//! User-facing board, report and conversation state. These are presentations,
//! never an alternate authority for canonical evidence or accepted facts.
use chrono::Utc;
use rusqlite::{params, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use crate::{ContinuityStore, CommandContext, CoreError, Result};
use crate::store::{append_event_with_context, bounded_json, record_command_with_context, validate_command_context};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceDocument {
    pub key: String,
    pub kind: String,
    pub revision: i64,
    pub payload: Value,
    pub updated_at: String,
}

fn validate_key(key: &str) -> Result<()> {
    if key.is_empty() || key.len() > 200 || !key.bytes().all(|b| b.is_ascii_alphanumeric() || b"-_:".contains(&b)) {
        return Err(CoreError::Validation("Invalid workspace document key".into()));
    }
    Ok(())
}

impl ContinuityStore {
    pub fn recent_workspace_documents(&self, prefix: &str) -> Result<Vec<WorkspaceDocument>> {
        validate_key(prefix)?;
        let connection=self.connection()?;
        let mut statement=connection.prepare("SELECT document_key FROM workspace_documents WHERE project_id=?1 AND substr(document_key,1,length(?2))=?2 ORDER BY updated_at DESC LIMIT 20")?;
        let keys=statement.query_map(params![self.manifest.project_id,prefix],|r|r.get::<_,String>(0))?.collect::<std::result::Result<Vec<_>,_>>()?;
        keys.into_iter().filter_map(|key|self.workspace_document(&key).transpose()).collect()
    }
    pub fn workspace_document(&self, key: &str) -> Result<Option<WorkspaceDocument>> {
        validate_key(key)?;
        let row: Option<(String,i64,String,String)> = self.connection()?.query_row(
            "SELECT kind,revision,payload_json,updated_at FROM workspace_documents WHERE project_id=?1 AND document_key=?2",
            params![self.manifest.project_id,key], |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?)),
        ).optional()?;
        row.map(|(kind,revision,payload,updated_at)| Ok(WorkspaceDocument {key:key.into(),kind,revision,payload:serde_json::from_str(&payload)?,updated_at})).transpose()
    }

    pub fn save_workspace_document(&self, command: &CommandContext, key: &str, kind: &str, expected_revision: i64, payload: &Value) -> Result<WorkspaceDocument> {
        validate_command_context(command)?;
        validate_key(key)?;
        if !["board","report","messages","preferences"].contains(&kind) || expected_revision < 0 || !payload.is_object() {
            return Err(CoreError::Validation("Invalid workspace document".into()));
        }
        let encoded = bounded_json(payload, 2 * 1024 * 1024, "workspace document")?;
        let mut connection = self.connection()?;
        let tx = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        crate::provenance::require_active_project(&tx, &self.manifest.project_id)?;
        let current: Option<(String,i64)> = tx.query_row("SELECT kind,revision FROM workspace_documents WHERE project_id=?1 AND document_key=?2",params![self.manifest.project_id,key],|r| Ok((r.get(0)?,r.get(1)?))).optional()?;
        if current.as_ref().map_or(0,|v|v.1) != expected_revision || current.as_ref().is_some_and(|v| v.0 != kind) {
            return Err(CoreError::Conflict("This workspace document changed elsewhere. Reload before saving; your draft has not been overwritten.".into()));
        }
        let revision = expected_revision + 1;
        let now = Utc::now().to_rfc3339();
        tx.execute("INSERT INTO workspace_documents(project_id,document_key,kind,revision,payload_json,updated_at) VALUES(?1,?2,?3,?4,?5,?6) ON CONFLICT(project_id,document_key) DO UPDATE SET revision=excluded.revision,payload_json=excluded.payload_json,updated_at=excluded.updated_at",params![self.manifest.project_id,key,kind,revision,encoded,now])?;
        tx.execute("INSERT INTO workspace_document_revisions(project_id,document_key,revision,payload_json,created_at) VALUES(?1,?2,?3,?4,?5)",params![self.manifest.project_id,key,revision,encoded,now])?;
        let metadata = json!({"document_key":key,"kind":kind,"revision":revision});
        record_command_with_context(&tx,command,&self.manifest.project_id,"SaveWorkspaceDocument",None,Some(expected_revision),&metadata)?;
        append_event_with_context(&tx,&self.manifest.project_id,command,None,"WorkspaceDocumentSaved",&metadata)?;
        tx.commit()?;
        Ok(WorkspaceDocument{key:key.into(),kind:kind.into(),revision,payload:payload.clone(),updated_at:now})
    }

    pub fn workspace_document_revision(&self, key: &str, revision: i64) -> Result<Value> {
        validate_key(key)?;
        let raw: String = self.connection()?.query_row("SELECT payload_json FROM workspace_document_revisions WHERE project_id=?1 AND document_key=?2 AND revision=?3",params![self.manifest.project_id,key,revision],|r|r.get(0))?;
        Ok(serde_json::from_str(&raw)?)
    }
}
