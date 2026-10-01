//! GitHub browser login and repository discovery via the official GitHub CLI.
//! Continuum never reads, stores, or serializes the CLI's credential token.
use std::io::Read;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;

use serde::{Deserialize, Serialize};

#[derive(Clone, Default, Serialize)]
pub struct LoginProgress {
    pub phase: String,
    pub user_code: Option<String>,
    pub message: Option<String>,
}

#[derive(Default)]
pub struct LoginState(pub Arc<Mutex<LoginProgress>>);

#[derive(Serialize)]
pub struct GithubStatus {
    pub cli_available: bool,
    pub connected: bool,
    pub username: Option<String>,
}

#[derive(Serialize, Deserialize)]
pub struct GithubRepository {
    pub full_name: String,
    pub clone_url: String,
    pub default_branch: String,
    pub private: bool,
}

#[derive(Deserialize)]
struct ApiRepository {
    full_name: String,
    clone_url: String,
    default_branch: Option<String>,
    private: bool,
    archived: bool,
    permissions: Option<ApiPermissions>,
}

#[derive(Deserialize)]
struct ApiPermissions { push: bool }

#[derive(Deserialize)]
struct ApiBranch { name: String }

pub fn is_available() -> bool {
    Command::new("gh").arg("--version").stdout(Stdio::null()).stderr(Stdio::null())
        .status().is_ok_and(|status| status.success())
}

fn config_dir() -> Result<PathBuf, String> {
    let base = std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))
        .ok_or("Cannot locate the user's configuration directory.")?;
    let directory = base.join("continuum").join("github-cli");
    std::fs::create_dir_all(&directory).map_err(|_| "Cannot create Continuum's GitHub login directory.".to_string())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o700))
            .map_err(|_| "Cannot protect Continuum's GitHub login directory.".to_string())?;
    }
    Ok(directory)
}

pub fn configure_git_command(command: &mut Command) -> bool {
    if let Ok(directory) = config_dir() {
        command.env("GH_CONFIG_DIR", directory)
            .env_remove("GH_TOKEN").env_remove("GITHUB_TOKEN");
        true
    } else { false }
}

fn gh_command() -> Result<Command, String> {
    let mut command = Command::new("gh");
    command.env("GH_CONFIG_DIR", config_dir()?)
        .env_remove("GH_TOKEN").env_remove("GITHUB_TOKEN")
        .env("GH_HOST", "github.com");
    Ok(command)
}

fn gh(args: &[&str]) -> Result<String, String> {
    let output = gh_command()?.args(args)
        .env("GH_PROMPT_DISABLED", "1")
        .stdout(Stdio::piped()).stderr(Stdio::null())
        .output().map_err(|_| "GitHub CLI is not installed. Install `gh` to enable browser login.".to_string())?;
    if !output.status.success() {
        return Err("GitHub is not connected, or repository access was denied. Use Login with GitHub and retry.".into());
    }
    String::from_utf8(output.stdout).map_err(|_| "GitHub returned unreadable data.".into())
}

fn connected() -> bool {
    gh(&["auth", "status", "--active", "--hostname", "github.com"]).is_ok()
}

fn code_in(text: &str) -> Option<String> {
    let bytes = text.as_bytes();
    bytes.windows(9).find(|slice| {
        slice[4] == b'-' && slice[..4].iter().chain(slice[5..].iter())
            .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit())
    }).and_then(|slice| std::str::from_utf8(slice).ok().map(ToOwned::to_owned))
}

fn open_authorization_page() -> Result<(), String> {
    #[cfg(target_os = "linux")]
    {
        Command::new("xdg-open").arg("https://github.com/login/device")
            .env_remove("APPDIR").env_remove("APPIMAGE")
            .env_remove("LD_LIBRARY_PATH").env_remove("LD_PRELOAD")
            .stdout(Stdio::null()).stderr(Stdio::null()).spawn()
            .map(|_| ()).map_err(|_| "Could not open the default browser. Open https://github.com/login/device manually.".into())
    }
    #[cfg(not(target_os = "linux"))]
    { Err("Open https://github.com/login/device in your browser.".into()) }
}

fn read_login_output(mut reader: impl Read, progress: Arc<Mutex<LoginProgress>>) {
    let mut buffer = [0_u8; 512];
    let mut recent = String::new();
    while let Ok(read) = reader.read(&mut buffer) {
        if read == 0 { break; }
        recent.push_str(&String::from_utf8_lossy(&buffer[..read]));
        if recent.len() > 4096 {
            recent = recent.chars().rev().take(1024).collect::<String>().chars().rev().collect();
        }
        if let Some(code) = code_in(&recent) {
            let should_open = if let Ok(mut state) = progress.lock() {
                let first_code = state.user_code.is_none();
                state.user_code = Some(code);
                first_code
            } else { false };
            if should_open {
                if let Err(message) = open_authorization_page() {
                    if let Ok(mut state) = progress.lock() { state.message = Some(message); }
                }
            }
        }
    }
}

#[tauri::command]
pub fn github_status() -> GithubStatus {
    let cli_available = is_available();
    let connected = cli_available && connected();
    let username = if connected {
        gh(&["api", "user", "--jq", ".login"]).ok().map(|name| name.trim().to_string())
            .filter(|name| !name.is_empty())
    } else { None };
    GithubStatus { cli_available, connected, username }
}

#[tauri::command]
pub fn github_begin_login(state: tauri::State<'_, LoginState>) -> Result<LoginProgress, String> {
    if !is_available() { return Err("GitHub CLI is not installed. On Fedora, install the `gh` package, then retry.".into()); }
    if connected() { return Ok(LoginProgress { phase: "connected".into(), ..Default::default() }); }
    let progress = Arc::clone(&state.0);
    {
        let mut current = progress.lock().map_err(|_| "Login state is unavailable.")?;
        if current.phase == "waiting" { return Ok(current.clone()); }
        *current = LoginProgress { phase: "waiting".into(), user_code: None, message: None };
    }
    thread::spawn(move || {
        let result = gh_command().and_then(|mut command| command
            .args(["auth", "login", "--hostname", "github.com", "--web", "--git-protocol", "https", "--skip-ssh-key"])
            .env("GH_BROWSER", "true")
            .stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn()
            .map_err(|_| "Could not start GitHub login.".to_string()));
        let Ok(mut child) = result else {
            if let Ok(mut current) = progress.lock() { current.phase = "error".into(); current.message = Some("Could not start GitHub login.".into()); }
            return;
        };
        // gh can ask for Enter before opening the browser. Extract only its
        // short-lived one-time code for the UI; never expose other CLI output.
        if let Some(mut input) = child.stdin.take() { let _ = std::io::Write::write_all(&mut input, b"\n"); }
        let out = child.stdout.take().map(|reader| { let state = Arc::clone(&progress); thread::spawn(move || read_login_output(reader, state)) });
        let err = child.stderr.take().map(|reader| { let state = Arc::clone(&progress); thread::spawn(move || read_login_output(reader, state)) });
        let success = child.wait().is_ok_and(|exit| exit.success());
        if let Some(reader) = out { let _ = reader.join(); }
        if let Some(reader) = err { let _ = reader.join(); }
        if let Ok(mut current) = progress.lock() {
            if success && connected() {
                current.phase = "connected".into(); current.message = None;
            } else {
                current.phase = "error".into();
                current.message = Some("GitHub login did not finish. Check the browser authorization and system clipboard, then retry.".into());
            }
        }
    });
    Ok(LoginProgress { phase: "waiting".into(), ..Default::default() })
}

#[tauri::command]
pub fn github_login_progress(state: tauri::State<'_, LoginState>) -> Result<LoginProgress, String> {
    state.0.lock().map(|current| current.clone()).map_err(|_| "Login state is unavailable.".into())
}

#[tauri::command]
pub fn github_open_authorization_page() -> Result<(), String> {
    open_authorization_page()
}

#[tauri::command]
pub fn github_list_repositories() -> Result<Vec<GithubRepository>, String> {
    let output = gh(&["api", "--paginate", "--slurp", "user/repos?per_page=100&affiliation=owner,collaborator,organization_member&sort=updated"])?;
    let pages: Vec<Vec<ApiRepository>> = serde_json::from_str(&output)
        .map_err(|_| "GitHub returned an unexpected repository list.")?;
    Ok(pages.into_iter().flatten().filter(|repo| {
        repo.private && !repo.archived && repo.permissions.as_ref().is_some_and(|permission| permission.push)
    }).filter_map(|repo| {
        let expected = format!("https://github.com/{}.git", repo.full_name);
        (repo.clone_url == expected && valid_repo_name(&repo.full_name)).then_some(GithubRepository {
            full_name: repo.full_name,
            clone_url: repo.clone_url,
            default_branch: repo.default_branch.unwrap_or_else(|| "main".into()),
            private: repo.private,
        })
    }).take(500).collect())
}

fn valid_repo_name(name: &str) -> bool {
    let segments: Vec<_> = name.split('/').collect();
    segments.len() == 2 && segments.iter().all(|part| {
        !part.is_empty() && !part.starts_with('-') && !matches!(*part, "." | "..")
            && part.chars().all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '.'))
    })
}

#[tauri::command]
pub fn github_list_branches(full_name: String) -> Result<Vec<String>, String> {
    if !valid_repo_name(&full_name) { return Err("Invalid repository name.".into()); }
    let endpoint = format!("repos/{full_name}/branches?per_page=100");
    let output = gh(&["api", "--paginate", "--slurp", &endpoint])?;
    let pages: Vec<Vec<ApiBranch>> = serde_json::from_str(&output)
        .map_err(|_| "GitHub returned an unexpected branch list.")?;
    Ok(pages.into_iter().flatten().map(|branch| branch.name).take(500).collect())
}

#[tauri::command]
pub fn github_open_new_repository() -> Result<(), String> {
    #[cfg(target_os = "linux")]
    {
        Command::new("xdg-open").arg("https://github.com/new")
            .stdout(Stdio::null()).stderr(Stdio::null()).spawn()
            .map(|_| ()).map_err(|_| "Could not open the browser. Visit https://github.com/new manually.".into())
    }
    #[cfg(not(target_os = "linux"))]
    { Err("Open https://github.com/new in your browser.".into()) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn device_code_is_extracted_without_exposing_other_output() {
        assert_eq!(code_in("First copy ABCD-EFGH into your browser"), Some("ABCD-EFGH".into()));
        assert_eq!(code_in("token ghp_secret"), None);
    }

    #[test]
    fn repository_names_cannot_be_cli_arguments_or_urls() {
        assert!(valid_repo_name("owner/project-name"));
        assert!(!valid_repo_name("owner/../project"));
        assert!(!valid_repo_name("--help/project"));
    }
}
