//! A Git remote is a versioned copy of a project, never the live project directory.
//! All Git commands run in a disposable clone; an unsuccessful push cannot rewrite local data.
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use continuum_core::{ContinuityStore, ProjectManifest};
use serde::{Deserialize, Serialize};
use tauri::Manager;

const LINK_FILE: &str = ".continuum-remote.json";
const SNAPSHOT_DIR: &str = "project";
const ATTRIBUTES: &str = "project/ledger.sqlite3 filter=lfs diff=lfs merge=lfs -text\nproject/artifacts/** filter=lfs diff=lfs merge=lfs -text\n";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteLink {
    version: u8,
    project_id: String,
    remote_url: String,
    branch: String,
    last_head: String,
}

#[derive(Serialize)]
pub struct RemoteStatus {
    pub remote_url: Option<String>,
    pub branch: Option<String>,
    pub last_head: Option<String>,
    pub git_available: bool,
    pub lfs_available: bool,
    pub estimated_current_bytes: u64,
}

#[derive(Serialize)]
pub struct PublishResult {
    pub remote_url: String,
    pub commit_id: String,
    pub changed: bool,
}

struct TemporaryTransfer(PathBuf);
impl Drop for TemporaryTransfer {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn transfer_dir(app: &tauri::AppHandle) -> Result<TemporaryTransfer, String> {
    let root = app
        .path()
        .app_cache_dir()
        .map_err(|error| error.to_string())?
        .join("project-transfers");
    fs::create_dir_all(&root).map_err(|error| error.to_string())?;
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| error.to_string())?
        .as_nanos();
    let path = root.join(format!("transfer-{}-{stamp}", std::process::id()));
    fs::create_dir(&path).map_err(|error| error.to_string())?;
    Ok(TemporaryTransfer(path))
}

fn valid_remote_url(raw: &str) -> Result<&str, String> {
    let value = raw.trim();
    if value != raw
        || value.chars().any(char::is_whitespace)
        || value.contains(['?', '#', '%', '\\'])
    {
        return Err("Use a plain GitHub or GitLab HTTPS/SSH repository URL without credentials or query parameters.".into());
    }
    let (host, path) = if let Some(rest) = value.strip_prefix("https://") {
        rest.split_once('/')
            .ok_or("Repository URL must include owner and repository.")?
    } else if let Some(rest) = value.strip_prefix("git@") {
        rest.split_once(':')
            .ok_or("SSH URL must include owner and repository.")?
    } else {
        return Err("Use https://github.com/…, https://gitlab.com/…, or git@host:…; never put a token in the URL.".into());
    };
    if !matches!(host, "github.com" | "gitlab.com") {
        return Err(
            "Only github.com and gitlab.com repositories are supported in this release.".into(),
        );
    }
    let segments: Vec<_> = path.split('/').collect();
    if segments.len() < 2
        || (host == "github.com" && segments.len() != 2)
        || segments.iter().any(|segment| {
            segment.is_empty()
                || *segment == "."
                || *segment == ".."
                || !segment
                    .chars()
                    .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.'))
        })
        || segments.last().is_some_and(|segment| *segment == ".git")
    {
        return Err("Repository path must contain a valid owner/group and repository name.".into());
    }
    Ok(value)
}

fn git(directory: Option<&Path>, args: &[&str]) -> Result<String, String> {
    let mut command = Command::new("git");
    command.args(args).env("GIT_TERMINAL_PROMPT", "0");
    // Scope the GitHub CLI credential helper to this subprocess and its Git LFS
    // children. Do not change the user's global Git configuration or expose a
    // token in URLs, arguments, environment variables, or project snapshots.
    #[cfg(target_os = "linux")]
    if crate::github_cli::is_available() && crate::github_cli::configure_git_command(&mut command) {
        command
            .env("GIT_CONFIG_COUNT", "1")
            .env("GIT_CONFIG_KEY_0", "credential.https://github.com.helper")
            .env("GIT_CONFIG_VALUE_0", "!gh auth git-credential");
    }
    if let Some(directory) = directory {
        command.current_dir(directory);
    }
    let output = command
        .output()
        .map_err(|error| format!("Git is unavailable: {error}"))?;
    if !output.status.success() {
        let detail = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(format!(
            "Git {} failed: {}",
            args.first().unwrap_or(&"command"),
            if detail.is_empty() {
                "check Git authentication and repository access"
            } else {
                &detail
            }
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn require_lfs() -> Result<(), String> {
    git(None, &["lfs", "version"]).map(|_| ()).map_err(|_| "Git LFS is required for complete project media. Install git-lfs, then retry; nothing was uploaded.".into())
}

fn read_link(root: &Path, project_id: &str) -> Result<Option<RemoteLink>, String> {
    let path = root.join(LINK_FILE);
    if !path.exists() {
        return Ok(None);
    }
    if path
        .symlink_metadata()
        .map_err(|error| error.to_string())?
        .file_type()
        .is_symlink()
    {
        return Err("Remote link file cannot be a symbolic link.".into());
    }
    let link: RemoteLink =
        serde_json::from_slice(&fs::read(path).map_err(|error| error.to_string())?)
            .map_err(|error| error.to_string())?;
    if link.version != 1
        || link.project_id != project_id
        || !link.last_head.chars().all(|ch| ch.is_ascii_hexdigit())
        || !matches!(link.last_head.len(), 40 | 64)
        || !valid_branch(&link.branch)
    {
        return Err("Saved repository link is invalid or belongs to another project.".into());
    }
    valid_remote_url(&link.remote_url)?;
    Ok(Some(link))
}

fn valid_branch(value: &str) -> bool {
    !value.is_empty()
        && value.len() < 120
        && value
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '/' | '.'))
        && !value.contains("..")
        && !value.ends_with('/')
}

fn write_link(root: &Path, link: &RemoteLink) -> Result<(), String> {
    let target = root.join(LINK_FILE);
    if target.exists()
        && target
            .symlink_metadata()
            .map_err(|error| error.to_string())?
            .file_type()
            .is_symlink()
    {
        return Err("Remote link file cannot be a symbolic link.".into());
    }
    let temporary = root.join(format!("{LINK_FILE}.{}-tmp", std::process::id()));
    fs::write(
        &temporary,
        serde_json::to_vec_pretty(link).map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())?;
    fs::rename(&temporary, &target).map_err(|error| error.to_string())
}

fn clone_remote(remote_url: &str, selected_branch: Option<&str>, destination: &Path) -> Result<(), String> {
    let target = destination
        .to_str()
        .ok_or("Transfer path is not valid UTF-8.")?;
    git(
        None,
        &[
            "clone",
            "--no-checkout",
            "--depth",
            "1",
            "--",
            remote_url,
            target,
        ],
    )?;
    if let Some(selected) = selected_branch {
        if !valid_branch(selected) { return Err("Selected branch name is invalid.".into()); }
        let current = branch(destination)?;
        if current != selected {
            if head(destination).is_some() {
                let ref_name = format!("refs/heads/{selected}");
                git(Some(destination), &["fetch", "--depth", "1", "origin", &ref_name])
                    .map_err(|_| "The selected branch does not exist on this repository.".to_string())?;
                let oid = git(Some(destination), &["rev-parse", "FETCH_HEAD"])?;
                git(Some(destination), &["update-ref", &ref_name, &oid])?;
            }
            git(Some(destination), &["symbolic-ref", "HEAD", &format!("refs/heads/{selected}")])?;
        }
    }
    Ok(())
}

fn head(repo: &Path) -> Option<String> {
    git(Some(repo), &["rev-parse", "--verify", "HEAD"]).ok()
}

fn branch(repo: &Path) -> Result<String, String> {
    let value = git(Some(repo), &["symbolic-ref", "--short", "HEAD"])?;
    if !valid_branch(&value) {
        return Err("Remote default branch has an unsupported name.".into());
    }
    Ok(value)
}

fn check_remote_head(
    prior: Option<&RemoteLink>,
    remote_url: &str,
    branch: &str,
    remote_head: Option<&str>,
) -> Result<(), String> {
    match prior {
        Some(link) if link.remote_url != remote_url => Err("This project is already linked to another repository. Its existing link was not changed.".into()),
        Some(link) if link.branch != branch || remote_head != Some(link.last_head.as_str()) => Err("The repository has changed since the last upload. Your local project is untouched. Review the remote version before choosing how to continue; no force-push was attempted.".into()),
        None if remote_head.is_some() => Err("For a first upload, connect an empty private repository. An existing remote version was found; your local project was not changed.".into()),
        _ => Ok(()),
    }
}

pub fn status(store: &ContinuityStore) -> Result<RemoteStatus, String> {
    let link = read_link(store.root(), &store.manifest().project_id)?;
    let mut estimated_current_bytes = fs::metadata(store.root().join("ledger.sqlite3"))
        .map_err(|error| error.to_string())?
        .len();
    let mut pending = vec![store.root().join("artifacts")];
    while let Some(directory) = pending.pop() {
        for entry in fs::read_dir(directory).map_err(|error| error.to_string())? {
            let entry = entry.map_err(|error| error.to_string())?;
            let kind = entry.file_type().map_err(|error| error.to_string())?;
            if kind.is_symlink() {
                return Err("Project artifacts contain a symbolic link; inspect project integrity before upload.".into());
            }
            if kind.is_dir() {
                pending.push(entry.path());
            }
            if kind.is_file() {
                estimated_current_bytes = estimated_current_bytes
                    .saturating_add(entry.metadata().map_err(|error| error.to_string())?.len());
            }
        }
    }
    Ok(RemoteStatus {
        remote_url: link.as_ref().map(|link| link.remote_url.clone()),
        branch: link.as_ref().map(|link| link.branch.clone()),
        last_head: link.map(|link| link.last_head),
        git_available: git(None, &["--version"]).is_ok(),
        lfs_available: git(None, &["lfs", "version"]).is_ok(),
        estimated_current_bytes,
    })
}

pub fn publish(
    app: &tauri::AppHandle,
    store: &ContinuityStore,
    remote_url: &str,
    selected_branch: Option<&str>,
    private_confirmed: bool,
) -> Result<PublishResult, String> {
    let remote_url = valid_remote_url(remote_url)?;
    if !private_confirmed {
        return Err("Confirm that the destination repository is private and the evidence is safe to upload.".into());
    }
    require_lfs()?;
    let prior = read_link(store.root(), &store.manifest().project_id)?;
    if prior.as_ref().is_some_and(|link| selected_branch.is_some_and(|branch| branch != link.branch)) {
        return Err("This project is linked to another branch; the existing link was not changed.".into());
    }
    if prior
        .as_ref()
        .is_some_and(|link| link.remote_url != remote_url)
    {
        return Err("This project is already linked to another repository. Its existing link was not changed.".into());
    }
    let transfer = transfer_dir(app)?;
    let repo = transfer.0.join("repo");
    clone_remote(remote_url, selected_branch, &repo)?;
    let remote_head = head(&repo);
    let branch = branch(&repo)?;
    check_remote_head(prior.as_ref(), remote_url, &branch, remote_head.as_deref())?;
    // Clone used --no-checkout: the destination is absent, while Git still knows the prior tree.
    store
        .export_project(repo.join(SNAPSHOT_DIR))
        .map_err(|error| error.to_string())?;
    fs::write(repo.join(".gitattributes"), ATTRIBUTES).map_err(|error| error.to_string())?;
    git(Some(&repo), &["lfs", "install", "--local"])?;
    git(Some(&repo), &["add", "--all"])?;
    let ledger_filter = git(
        Some(&repo),
        &["check-attr", "filter", "--", "project/ledger.sqlite3"],
    )?;
    if !ledger_filter.ends_with("filter: lfs") {
        return Err("Git LFS did not claim the project database; upload was stopped.".into());
    }
    let changes = git(Some(&repo), &["status", "--porcelain"])?;
    if changes.is_empty() {
        return Ok(PublishResult {
            remote_url: remote_url.into(),
            commit_id: remote_head.unwrap_or_default(),
            changed: false,
        });
    }
    git(Some(&repo), &["commit", "-m", "Continuum project snapshot"])?;
    let commit_id = git(Some(&repo), &["rev-parse", "HEAD"])?;
    git(
        Some(&repo),
        &["push", "origin", &format!("HEAD:refs/heads/{branch}")],
    )?;
    let link = RemoteLink {
        version: 1,
        project_id: store.manifest().project_id.clone(),
        remote_url: remote_url.into(),
        branch,
        last_head: commit_id.clone(),
    };
    write_link(store.root(), &link)?;
    Ok(PublishResult {
        remote_url: remote_url.into(),
        commit_id,
        changed: true,
    })
}

pub fn restore(
    app: &tauri::AppHandle,
    remote_url: &str,
    selected_branch: Option<&str>,
    destination: &Path,
) -> Result<ContinuityStore, String> {
    let remote_url = valid_remote_url(remote_url)?;
    require_lfs()?;
    if destination.exists() {
        return Err("Restore destination already exists; choose a new folder name.".into());
    }
    let transfer = transfer_dir(app)?;
    let repo = transfer.0.join("repo");
    clone_remote(remote_url, selected_branch, &repo)?;
    let remote_head = head(&repo).ok_or("Repository has no Continuum snapshot yet.")?;
    let branch = branch(&repo)?;
    git(Some(&repo), &["lfs", "install", "--local"])?;
    git(Some(&repo), &["checkout", "--force", "HEAD"])?;
    let source = repo.join(SNAPSHOT_DIR);
    if !source.join("continuum.project.json").is_file() {
        return Err("This branch does not contain a Continuum project snapshot. Upload the local project from Project settings first, or choose the branch used for an earlier upload.".into());
    }
    git(Some(&repo), &["lfs", "pull"])?;
    git(Some(&repo), &["lfs", "checkout"])?;
    let manifest = ProjectManifest::load(&source)
        .map_err(|error| format!("Repository has no valid Continuum project: {error}"))?;
    let imported =
        ContinuityStore::import_export(&source, destination).map_err(|error| error.to_string())?;
    let link = RemoteLink {
        version: 1,
        project_id: manifest.project_id,
        remote_url: remote_url.into(),
        branch,
        last_head: remote_head,
    };
    write_link(imported.root(), &link)?;
    Ok(imported)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_plain_provider_urls_are_accepted() {
        for url in [
            "https://github.com/wiefran/research.git",
            "https://gitlab.com/team/continuum/data.git",
            "git@gitlab.com:team/continuum/data.git",
        ] {
            assert!(valid_remote_url(url).is_ok(), "{url}");
        }
        for url in [
            "https://user:token@github.com/a/b.git",
            "file:///tmp/repo",
            "https://evil.example/a/b",
            "https://github.com/a/../b",
            "https://github.com/a/b?token=secret",
            "--upload-pack=evil",
        ] {
            assert!(valid_remote_url(url).is_err(), "{url}");
        }
    }

    #[test]
    fn branch_names_are_bounded() {
        assert!(valid_branch("main"));
        assert!(!valid_branch("../main"));
        assert!(!valid_branch("main:other"));
    }

    #[test]
    fn remote_changes_never_become_implicit_local_imports_or_force_pushes() {
        let link = RemoteLink {
            version: 1,
            project_id: "project".into(),
            remote_url: "https://github.com/a/b.git".into(),
            branch: "main".into(),
            last_head: "a".repeat(40),
        };
        assert!(
            check_remote_head(Some(&link), &link.remote_url, "main", Some(&link.last_head)).is_ok()
        );
        assert!(
            check_remote_head(Some(&link), &link.remote_url, "main", Some(&"b".repeat(40)))
                .is_err()
        );
        assert!(
            check_remote_head(
                Some(&link),
                &link.remote_url,
                "other",
                Some(&link.last_head)
            )
            .is_err()
        );
        assert!(check_remote_head(None, &link.remote_url, "main", Some(&link.last_head)).is_err());
        assert!(check_remote_head(None, &link.remote_url, "main", None).is_ok());
    }

    #[test]
    fn selected_existing_branch_is_loaded_without_touching_a_live_project() {
        let directory = tempfile::tempdir().unwrap();
        let bare = directory.path().join("remote.git");
        git(None, &["init", "--bare", "--initial-branch=main", bare.to_str().unwrap()]).unwrap();
        let seed = directory.path().join("seed");
        git(None, &["clone", bare.to_str().unwrap(), seed.to_str().unwrap()]).unwrap();
        git(Some(&seed), &["config", "user.name", "Continuum test"]).unwrap();
        git(Some(&seed), &["config", "user.email", "continuum-test@example.invalid"]).unwrap();
        fs::write(seed.join("marker.txt"), "main").unwrap();
        git(Some(&seed), &["add", "marker.txt"]).unwrap();
        git(Some(&seed), &["commit", "-m", "main"] ).unwrap();
        git(Some(&seed), &["push", "origin", "main"]).unwrap();
        git(Some(&seed), &["checkout", "-b", "research"]).unwrap();
        fs::write(seed.join("marker.txt"), "research").unwrap();
        git(Some(&seed), &["commit", "-am", "research"]).unwrap();
        git(Some(&seed), &["push", "origin", "research"]).unwrap();
        let selected = directory.path().join("selected");
        clone_remote(bare.to_str().unwrap(), Some("research"), &selected).unwrap();
        assert_eq!(branch(&selected).unwrap(), "research");
        assert_eq!(head(&selected), git(Some(&seed), &["rev-parse", "HEAD"]).ok());
        assert!(!selected.join("marker.txt").exists(), "clone must remain disposable and un-checked-out");
    }

    #[test]
    #[ignore = "requires Git LFS and a local Git remote"]
    fn snapshot_media_roundtrip_through_git_lfs() {
        require_lfs().unwrap();
        let directory = tempfile::tempdir().unwrap();
        let bare = directory.path().join("remote.git");
        git(
            None,
            &[
                "init",
                "--bare",
                "--initial-branch=main",
                bare.to_str().unwrap(),
            ],
        )
        .unwrap();
        let live =
            ContinuityStore::create(directory.path().join("live"), "Portable research").unwrap();
        let media = b"recorded video bytes remain unchanged";
        let artifact = live
            .ingest_artifact(&continuum_core::new_id(), media, "video/webm")
            .unwrap();

        let first = directory.path().join("first-clone");
        clone_remote(bare.to_str().unwrap(), None, &first).unwrap();
        live.export_project(first.join(SNAPSHOT_DIR)).unwrap();
        fs::write(first.join(".gitattributes"), ATTRIBUTES).unwrap();
        git(Some(&first), &["lfs", "install", "--local"]).unwrap();
        git(Some(&first), &["add", "--all"]).unwrap();
        assert!(
            git(
                Some(&first),
                &[
                    "check-attr",
                    "filter",
                    "--",
                    &format!("project/{}", artifact.relative_path)
                ]
            )
            .unwrap()
            .ends_with("filter: lfs")
        );
        git(
            Some(&first),
            &["commit", "-m", "Initial Continuum snapshot"],
        )
        .unwrap();
        git(Some(&first), &["push", "origin", "HEAD:refs/heads/main"]).unwrap();

        let second = directory.path().join("second-clone");
        clone_remote(bare.to_str().unwrap(), None, &second).unwrap();
        git(Some(&second), &["lfs", "install", "--local"]).unwrap();
        git(Some(&second), &["checkout", "--force", "HEAD"]).unwrap();
        git(Some(&second), &["lfs", "pull"]).unwrap();
        git(Some(&second), &["lfs", "checkout"]).unwrap();
        let restored_db = fs::read(second.join("project/ledger.sqlite3")).unwrap();
        assert!(
            restored_db.starts_with(b"SQLite format 3"),
            "database is still a pointer: {:?}; LFS files: {}",
            String::from_utf8_lossy(&restored_db[..restored_db.len().min(160)]),
            git(Some(&second), &["lfs", "ls-files"]).unwrap()
        );
        let restored = ContinuityStore::import_export(
            second.join(SNAPSHOT_DIR),
            directory.path().join("restored"),
        )
        .unwrap();
        assert_eq!(restored.manifest().project_id, live.manifest().project_id);
        assert_eq!(
            restored.read_artifact_bounded(&artifact.id, 1024).unwrap(),
            media
        );
        assert!(restored.verify_integrity().unwrap().is_healthy());
    }
}
