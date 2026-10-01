//! Platform integration shared by Git, GitHub and AI clients.
use std::path::{Path, PathBuf};
use std::process::Command;

#[cfg(windows)]
pub fn valid_windows_folder_name(name: &str) -> bool {
    let base = name.split('.').next().unwrap_or("").to_uppercase();
    !name.ends_with(['.', ' ']) && !name.chars().any(|c| c.is_control() || "<>:\"/\\|?*".contains(c))
        && !matches!(base.as_str(), "CON" | "PRN" | "AUX" | "NUL" | "CLOCK$")
        && !(base.len() == 4 && (base.starts_with("COM") || base.starts_with("LPT")) && matches!(base.as_bytes()[3], b'1'..=b'9'))
}

#[cfg(all(test, windows))]
mod tests {
    #[test]
    fn windows_project_folders_preserve_safe_unicode_and_reject_device_aliases() {
        for name in ["Riset Wiefran", "Eksperimen-é", "プロジェクト", "comet"] { assert!(super::valid_windows_folder_name(name)); }
        for name in ["CON", "nul.txt", "Lpt9.data", "com1", "folder.", "file:stream", "a*b", "a?b"] { assert!(!super::valid_windows_folder_name(name)); }
    }
}

pub fn tool(name: &str) -> PathBuf {
    #[cfg(windows)]
    {
        if let Ok(executable) = std::env::current_exe() {
            if let Some(root) = executable.parent() {
                let relative = match name {
                    "git" => "tools/git/cmd/git.exe",
                    "gh" => "tools/gh/bin/gh.exe",
                    _ => "",
                };
                let bundled = root.join(relative);
                if !relative.is_empty() && bundled.is_file() {
                    return bundled;
                }
            }
        }
        let candidates = std::env::var_os("PATH")
            .into_iter()
            .flat_map(|value| std::env::split_paths(&value).collect::<Vec<_>>())
            .map(|root| root.join(format!("{name}.exe")));
        let installed = ["ProgramFiles", "ProgramFiles(x86)", "LOCALAPPDATA"]
            .into_iter()
            .filter_map(std::env::var_os)
            .flat_map(|root| {
                let root = PathBuf::from(root);
                match name {
                    "git" => vec![
                        root.join("Git/cmd/git.exe"),
                        root.join("Programs/Git/cmd/git.exe"),
                    ],
                    "gh" => vec![
                        root.join("GitHub CLI/gh.exe"),
                        root.join("Programs/GitHub CLI/gh.exe"),
                    ],
                    _ => vec![],
                }
            });
        if let Some(candidate) = candidates.chain(installed).find(|path| path.is_file()) {
            return candidate;
        }
    }
    PathBuf::from(name)
}

pub fn command(program: impl AsRef<Path>) -> Command {
    #[allow(unused_mut)]
    let mut command = Command::new(program.as_ref());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000); // CREATE_NO_WINDOW
        let codex = program.as_ref().file_name().is_some_and(|name| {
            matches!(name.to_string_lossy().to_ascii_lowercase().as_str(), "codex" | "codex.exe" | "codex.cmd")
        });
        if codex && std::env::var_os("CODEX_HOME").is_none() {
            if let Some(profile) = std::env::var_os("USERPROFILE") {
                let profile = PathBuf::from(profile);
                if profile.is_absolute() { command.env("CODEX_HOME", profile.join(".codex")); }
            }
        }
        // git-credential and git-lfs are subprocesses, so expose the bundled tools
        // only to this child; never alter the user's PATH or global Git config.
        if let Ok(executable) = std::env::current_exe() {
            if let Some(root) = executable.parent() {
                let mut paths = vec![
                    root.join("tools/gh/bin"),
                    root.join("tools/git/cmd"),
                    root.join("tools/git/ucrt64/bin"),
                    root.join("tools/git/mingw64/bin"),
                    root.join("tools/git/usr/bin"),
                ];
                if let Some(path) = std::env::var_os("PATH") {
                    paths.extend(std::env::split_paths(&path));
                }
                if let Ok(path) = std::env::join_paths(paths) {
                    command.env("PATH", path);
                }
            }
        }
    }
    command
}

pub fn open_github(url: &str) -> Result<(), String> {
    if !matches!(
        url,
        "https://github.com/login/device" | "https://github.com/new"
    ) {
        return Err("Unsupported GitHub browser destination.".into());
    }
    #[cfg(windows)]
    {
        use windows_sys::Win32::UI::{Shell::ShellExecuteW, WindowsAndMessaging::SW_SHOWNORMAL};
        let wide: Vec<u16> = url.encode_utf16().chain(Some(0)).collect();
        let result = unsafe {
            ShellExecuteW(
                std::ptr::null_mut(),
                std::ptr::null(),
                wide.as_ptr(),
                std::ptr::null(),
                std::ptr::null(),
                SW_SHOWNORMAL,
            )
        };
        if result as isize <= 32 {
            Err(format!(
                "Windows could not open the browser. Open {url} manually."
            ))
        } else {
            Ok(())
        }
    }
    #[cfg(not(windows))]
    {
        let mut child = command("xdg-open")
            .arg(url)
            .env_remove("APPDIR")
            .env_remove("APPIMAGE")
            .env_remove("LD_LIBRARY_PATH")
            .env_remove("LD_PRELOAD")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .map_err(|_| format!("Could not open the browser. Open {url} manually."))?;
        std::thread::spawn(move || {
            let _ = child.wait();
        });
        Ok(())
    }
}
