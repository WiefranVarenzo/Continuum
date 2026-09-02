use std::fs;
use std::path::{Path, PathBuf};

use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::{CoreError, PROJECT_MANIFEST_VERSION, Result};

pub const MANIFEST_FILE: &str = "continuum.project.json";
pub const LEDGER_FILE: &str = "ledger.sqlite3";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProjectManifest {
    pub manifest_version: u32,
    pub project_id: String,
    pub name: String,
    pub created_at: String,
    pub ledger: String,
    pub artifact_algorithm: String,
}

impl ProjectManifest {
    pub(crate) fn new(project_id: String, name: String) -> Self {
        Self {
            manifest_version: PROJECT_MANIFEST_VERSION,
            project_id,
            name,
            created_at: Utc::now().to_rfc3339(),
            ledger: LEDGER_FILE.into(),
            artifact_algorithm: "sha256".into(),
        }
    }

    pub fn load(root: &Path) -> Result<Self> {
        let manifest: Self = serde_json::from_slice(&fs::read(root.join(MANIFEST_FILE))?)?;
        if manifest.manifest_version > PROJECT_MANIFEST_VERSION {
            return Err(CoreError::UnsupportedSchema {
                found: manifest.manifest_version,
                supported: PROJECT_MANIFEST_VERSION,
            });
        }
        validate_relative_file(&manifest.ledger)?;
        Ok(manifest)
    }

    pub(crate) fn write_atomic(&self, root: &Path) -> Result<()> {
        let target = root.join(MANIFEST_FILE);
        let temporary = root.join(format!(".{MANIFEST_FILE}.tmp"));
        let bytes = serde_json::to_vec_pretty(self)?;
        fs::write(&temporary, bytes)?;
        fs::rename(temporary, target)?;
        Ok(())
    }

    pub(crate) fn ledger_path(&self, root: &Path) -> PathBuf {
        root.join(&self.ledger)
    }
}

fn validate_relative_file(value: &str) -> Result<()> {
    let path = Path::new(value);
    if path.is_absolute()
        || path.components().count() != 1
        || value.contains("..")
        || value.contains('/')
        || value.contains('\\')
    {
        return Err(CoreError::Validation(
            "manifest ledger path must be a project-local file".into(),
        ));
    }
    Ok(())
}
