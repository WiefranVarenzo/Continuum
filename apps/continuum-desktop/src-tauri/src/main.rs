#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

use std::fs;
use std::path::{Component, Path, PathBuf};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use continuum_core::{
    ActorRef, AnalysisLimits, AnalyzeBaselineInput, ArtifactClassification, CORE_SCHEMA_VERSION,
    CaptureExternalEvidence, CaptureMarker, CapturePermissionStatus, CaptureSegment,
    CaptureSession, CaptureSessionPage, CaptureSourceKind, CheckpointComparison,
    CheckpointEnvelope, CheckpointEnvelopePage, CheckpointScope, CommandContext,
    CommitIngestionInput, ContextPack, ContextPackRequest, ContinuityStore, CreatedMcpGrant,
    CurrentProjectState, CurrentProjectStateRequest, ExternalProposal, ExternalProposalKind,
    ExternalProposalPage, FindingSource, FindingSourceAssessment,
    HumanDocument, HumanDocumentFormat, HumanDocumentKind, HumanDocumentPage, HumanDocumentRequest,
    CoreError, IntegrityIssue, McpAuditPage, McpClientGrant, McpGrantPage, NewCaptureMarker,
    NewCaptureSegment, NewCaptureSegmentEvidence, NewCaptureSession, NewMcpClientGrant,
    NewFinding, NewResearchQuestion, PageRequest, ProposalReviewDecision, QuestionKind,
    RepositoryAttachInput, ResearchEntityKind, ResearchItem, ResearchSearchPage,
    ResearchSearchQuery, SavedContextPack, SavedContextPackPage, SemanticCheckpointInput, Space,
};
use serde::Serialize;
use tauri::State;

mod workspace;
mod assistant;
mod remote_sync;
mod github_cli;
mod platform;
#[cfg(not(windows))]
mod system_audio;
#[cfg(windows)]
#[path = "system_audio_windows.rs"]
mod system_audio;
#[cfg(target_os = "linux")]
mod media_permissions;
#[cfg(windows)]
#[path = "media_permissions_windows.rs"]
mod media_permissions;
#[cfg(target_os = "linux")]
mod recording_runtime;
#[cfg(target_os = "linux")]
mod native_screen;

#[derive(Default)]
struct AppState {
    store: Mutex<Option<ContinuityStore>>,
}

#[derive(Serialize)]
struct OpenedProject {
    project_id: String,
    name: String,
    path: String,
    status: String,
    ledger_sequence: i64,
    research: bool,
    development: bool,
    integrity_healthy: bool,
}

#[derive(Serialize)]
struct ReleaseDiagnostics {
    app_version: &'static str,
    schema_version: u32,
    project_id: String,
    project_name: String,
    project_path: String,
    project_status: String,
    ledger_sequence: i64,
    research_enabled: bool,
    development_enabled: bool,
    checked_artifacts: usize,
    integrity_healthy: bool,
    issues: Vec<IntegrityIssue>,
}

#[derive(Serialize)]
struct CreatedPath {
    path: String,
}

#[derive(Serialize)]
struct RepositorySyncSummary {
    repository_id: String,
    baseline_id: String,
    branch_name: Option<String>,
    head_oid: Option<String>,
    worktree_changes: usize,
    ingested_commits: usize,
    remaining_commits: usize,
    analyzed_files: usize,
    code_entities: usize,
    discovered_tests: usize,
    analysis_completeness: String,
}

#[derive(Serialize)]
struct CapturePreview {
    artifact_id: String,
    media_type: String,
    bytes: Vec<u8>,
}

#[derive(Serialize)]
struct PublishedDesktopDocument {
    artifact_id: String,
    file_path: String,
}

#[derive(Serialize)]
struct CaptureRuntimeHealth {
    platform: &'static str,
    recording_backend_available: Option<bool>,
    missing_recording_elements: Vec<String>,
    native_screen_available: bool,
    missing_native_screen_elements: Vec<String>,
    native_screen_audio_available: bool,
    missing_native_screen_audio_elements: Vec<String>,
    native_system_audio_available: bool,
    scanner_ready: bool,
    pipewire_socket_ready: bool,
    desktop_portal_session_ready: bool,
    summary: String,
}

#[derive(Serialize)]
struct SpaceCapabilities {
    research: bool,
    development: bool,
}

fn desktop_user_command() -> CommandContext {
    CommandContext::new(ActorRef::user("desktop-user"))
}

fn safe_child(parent: &Path, directory_name: &str) -> Result<PathBuf, String> {
    let directory_name = directory_name.trim();
    if directory_name.is_empty() || directory_name.chars().count() > 120 {
        return Err("folder name must contain 1..=120 characters".into());
    }
    let mut components = Path::new(directory_name).components();
    if !matches!(components.next(), Some(Component::Normal(_))) || components.next().is_some() {
        return Err("folder name must be one safe directory name".into());
    }
    #[cfg(windows)]
    if !platform::valid_windows_folder_name(directory_name) {
        return Err("Choose a Windows folder name without reserved device names, trailing dots, or < > : \" / \\ | ? * characters.".into());
    }
    let parent = fs::canonicalize(parent).map_err(|error| error.to_string())?;
    if !parent.is_dir() {
        return Err("selected parent path is not a directory".into());
    }
    Ok(parent.join(directory_name))
}

fn portable_slug(value: &str) -> String {
    let slug = value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>();
    let slug = slug
        .split('-')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("-");
    if slug.is_empty() {
        "continuum-project".into()
    } else {
        slug
    }
}

fn timestamp_millis() -> Result<u128, String> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .map_err(|_| "system clock is before the Unix epoch".into())
}

fn opened_project(store: &ContinuityStore) -> Result<OpenedProject, String> {
    let summary = store.summary().map_err(|error| error.to_string())?;
    let integrity = store
        .verify_integrity()
        .map_err(|error| error.to_string())?;
    Ok(OpenedProject {
        project_id: summary.project_id,
        name: summary.name,
        path: store.root().to_string_lossy().into_owned(),
        status: summary.status,
        ledger_sequence: summary.ledger_sequence,
        research: store
            .capability_enabled(Space::Research)
            .map_err(|error| error.to_string())?,
        development: store
            .capability_enabled(Space::Development)
            .map_err(|error| error.to_string())?,
        integrity_healthy: integrity.is_healthy(),
    })
}

fn install_store(
    store: ContinuityStore,
    state: &State<'_, AppState>,
) -> Result<OpenedProject, String> {
    let project = opened_project(&store)?;
    *state
        .store
        .lock()
        .map_err(|_| "project state lock is poisoned".to_string())? = Some(store);
    Ok(project)
}

fn with_store<T>(
    state: &State<'_, AppState>,
    action: impl FnOnce(&ContinuityStore) -> continuum_core::Result<T>,
) -> Result<T, String> {
    let guard = state
        .store
        .lock()
        .map_err(|_| "project state lock is poisoned".to_string())?;
    let store = guard
        .as_ref()
        .ok_or_else(|| "open a Continuum project first".to_string())?;
    action(store).map_err(|error| error.to_string())
}

#[tauri::command]
fn open_project(path: PathBuf, state: State<'_, AppState>) -> Result<OpenedProject, String> {
    let store = ContinuityStore::open(path).map_err(|error| error.to_string())?;
    install_store(store, &state)
}

#[tauri::command]
fn create_project(
    parent_path: PathBuf,
    directory_name: String,
    name: String,
    research: bool,
    development: bool,
    state: State<'_, AppState>,
) -> Result<OpenedProject, String> {
    if !research && !development {
        return Err("enable Research, Development, or both".into());
    }
    let destination = safe_child(&parent_path, &directory_name)?;
    let store =
        ContinuityStore::create_with_actor(destination, name, ActorRef::user("desktop-user"))
            .map_err(|error| error.to_string())?;
    if research {
        store
            .set_space_capability_with_context(&desktop_user_command(), Space::Research, true)
            .map_err(|error| error.to_string())?;
    }
    if development {
        store
            .set_space_capability_with_context(&desktop_user_command(), Space::Development, true)
            .map_err(|error| error.to_string())?;
    }
    install_store(store, &state)
}

#[tauri::command]
fn close_project(app: tauri::AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    #[cfg(target_os = "linux")]
    tauri::Manager::state::<native_screen::NativeScreenState>(&app).cancel();
    #[cfg(not(target_os = "linux"))]
    let _ = app;
    *state
        .store
        .lock()
        .map_err(|_| "project state lock is poisoned".to_string())? = None;
    Ok(())
}

#[tauri::command]
fn run_release_diagnostics(state: State<'_, AppState>) -> Result<ReleaseDiagnostics, String> {
    with_store(&state, |store| {
        let summary = store.summary()?;
        let integrity = store.verify_integrity()?;
        Ok(ReleaseDiagnostics {
            app_version: env!("CARGO_PKG_VERSION"),
            schema_version: CORE_SCHEMA_VERSION,
            project_id: summary.project_id,
            project_name: summary.name,
            project_path: store.root().to_string_lossy().into_owned(),
            project_status: summary.status,
            ledger_sequence: summary.ledger_sequence,
            research_enabled: store.capability_enabled(Space::Research)?,
            development_enabled: store.capability_enabled(Space::Development)?,
            checked_artifacts: integrity.checked_artifacts,
            integrity_healthy: integrity.is_healthy(),
            issues: integrity.issues,
        })
    })
}

#[tauri::command]
fn backup_project(
    destination_parent: PathBuf,
    state: State<'_, AppState>,
) -> Result<CreatedPath, String> {
    with_store(&state, |store| {
        let parent = fs::canonicalize(&destination_parent)?;
        if !parent.is_dir() {
            return Err(continuum_core::CoreError::Validation(
                "backup destination must be a directory".into(),
            ));
        }
        let timestamp = timestamp_millis().map_err(continuum_core::CoreError::Validation)?;
        let destination = parent.join(format!(
            "{}-backup-{timestamp}.sqlite3",
            portable_slug(&store.manifest().name)
        ));
        let path = store.backup_database(destination)?;
        Ok(CreatedPath {
            path: path.to_string_lossy().into_owned(),
        })
    })
}

#[tauri::command]
fn export_project(
    destination_parent: PathBuf,
    state: State<'_, AppState>,
) -> Result<CreatedPath, String> {
    with_store(&state, |store| {
        let parent = fs::canonicalize(&destination_parent)?;
        if !parent.is_dir() {
            return Err(continuum_core::CoreError::Validation(
                "export destination must be a directory".into(),
            ));
        }
        let timestamp = timestamp_millis().map_err(continuum_core::CoreError::Validation)?;
        let destination = parent.join(format!(
            "{}-export-{timestamp}",
            portable_slug(&store.manifest().name)
        ));
        store.export_project(&destination)?;
        Ok(CreatedPath {
            path: destination.to_string_lossy().into_owned(),
        })
    })
}

#[tauri::command]
fn restore_project(
    source_path: PathBuf,
    destination_parent: PathBuf,
    directory_name: String,
    state: State<'_, AppState>,
) -> Result<OpenedProject, String> {
    let destination = safe_child(&destination_parent, &directory_name)?;
    let store = ContinuityStore::import_export(source_path, destination)
        .map_err(|error| error.to_string())?;
    install_store(store, &state)
}

#[tauri::command]
fn project_remote_status(state: State<'_, AppState>) -> Result<remote_sync::RemoteStatus, String> {
    with_store(&state, |store| remote_sync::status(store).map_err(CoreError::Validation))
}

#[tauri::command]
async fn publish_project_snapshot(
    remote_url: String,
    branch: Option<String>,
    private_confirmed: bool,
    app: tauri::AppHandle,
) -> Result<remote_sync::PublishResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = tauri::Manager::state::<AppState>(&app);
        // The store is a lightweight path-and-manifest handle. Release the UI
        // state lock before Git/LFS network operations can take minutes.
        let store = state.store.lock()
            .map_err(|_| "project state lock is poisoned".to_string())?
            .as_ref().cloned()
            .ok_or_else(|| "open a Continuum project first".to_string())?;
        remote_sync::publish(&app, &store, &remote_url, branch.as_deref(), private_confirmed)
    }).await.map_err(|error| format!("Upload worker stopped unexpectedly: {error}"))?
}

#[tauri::command]
async fn restore_project_from_remote(
    remote_url: String,
    branch: Option<String>,
    destination_parent: PathBuf,
    directory_name: String,
    app: tauri::AppHandle,
) -> Result<OpenedProject, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let destination = safe_child(&destination_parent, &directory_name)?;
        let store = remote_sync::restore(&app, &remote_url, branch.as_deref(), &destination)?;
        let state = tauri::Manager::state::<AppState>(&app);
        install_store(store, &state)
    }).await.map_err(|error| format!("Download worker stopped unexpectedly: {error}"))?
}

#[tauri::command]
fn create_workspace_question(
    title: String,
    question: String,
    context: String,
    desired_outcome: String,
    priority: u8,
    state: State<'_, AppState>,
) -> Result<ResearchItem, String> {
    with_store(&state, |store| {
        store.create_research_question(
            &desktop_user_command(),
            NewResearchQuestion {
                title,
                kind: QuestionKind::Question,
                question,
                context,
                desired_outcome,
                priority,
                due_at: None,
                session_id: None,
                metadata: serde_json::json!({"created_from":"desktop_workspace"}),
            },
        )
    })
}

#[tauri::command]
fn attach_and_sync_repository(
    path: PathBuf,
    state: State<'_, AppState>,
) -> Result<RepositorySyncSummary, String> {
    with_store(&state, |store| {
        let repository = store.attach_repository(
            &desktop_user_command(),
            RepositoryAttachInput {
                path,
                title: None,
                metadata: serde_json::json!({"attached_from":"desktop_workspace"}),
            },
        )?;
        let baseline =
            store.observe_repository_baseline(&desktop_user_command(), &repository.entity.id)?;
        let commits = store.ingest_git_commits(
            &desktop_user_command(),
            CommitIngestionInput {
                repository_id: repository.entity.id.clone(),
                baseline_id: baseline.entity.id.clone(),
                max_commits: 1_000,
            },
        )?;
        let analysis = store.analyze_repository_baseline(
            &desktop_user_command(),
            AnalyzeBaselineInput {
                repository_id: repository.entity.id.clone(),
                baseline_id: baseline.entity.id.clone(),
                limits: AnalysisLimits::default(),
            },
        )?;
        Ok(RepositorySyncSummary {
            repository_id: repository.entity.id,
            baseline_id: baseline.entity.id,
            branch_name: baseline.branch_name,
            head_oid: baseline.head_oid,
            worktree_changes: baseline.worktree_status.len(),
            ingested_commits: commits.ingested_commit_ids.len(),
            remaining_commits: commits.remaining_commits,
            analyzed_files: analysis.analyzed_file_count,
            code_entities: analysis.code_entity_count,
            discovered_tests: analysis.test_count,
            analysis_completeness: analysis.completeness,
        })
    })
}

#[tauri::command]
fn compose_human_document(
    request: HumanDocumentRequest,
    state: State<'_, AppState>,
) -> Result<HumanDocument, String> {
    with_store(&state, |store| store.compose_human_document(request))
}

#[tauri::command]
fn save_human_document(
    document: HumanDocument,
    state: State<'_, AppState>,
) -> Result<HumanDocument, String> {
    with_store(&state, |store| {
        store.save_human_document(&desktop_user_command(), &document)
    })
}

#[tauri::command]
fn list_human_documents(
    kind: Option<HumanDocumentKind>,
    limit: u32,
    offset: u64,
    state: State<'_, AppState>,
) -> Result<HumanDocumentPage, String> {
    with_store(&state, |store| {
        store.list_human_documents(kind, PageRequest { limit, offset })
    })
}

#[tauri::command]
fn publish_human_document(
    document_id: String,
    format: HumanDocumentFormat,
    state: State<'_, AppState>,
) -> Result<PublishedDesktopDocument, String> {
    with_store(&state, |store| {
        let document = store.get_human_document(&document_id)?;
        let extension = match format {
            HumanDocumentFormat::Html => "html",
            HumanDocumentFormat::Markdown => "md",
        };
        let published = store.publish_human_document(&desktop_user_command(), &document_id, format)?;
        let bytes = store.read_artifact_bounded(&published.artifact_id, 16 * 1024 * 1024)?;
        let directory = store.root().join("exports").join("reports");
        if directory.is_symlink() {
            return Err(CoreError::Validation("Report export folder must not be a symbolic link".into()));
        }
        fs::create_dir_all(&directory)?;
        let path = directory.join(format!(
            "{}-{}.{}",
            portable_slug(&document.title), published.export_id, extension
        ));
        if path.exists() {
            if path.is_symlink() || fs::read(&path)? != bytes {
                return Err(CoreError::Conflict("Existing report file differs from the verified export".into()));
            }
        } else {
            let mut file = fs::OpenOptions::new().write(true).create_new(true).open(&path)?;
            use std::io::Write as _;
            file.write_all(&bytes)?;
            file.sync_all()?;
        }
        Ok(PublishedDesktopDocument {
            artifact_id: published.artifact_id,
            file_path: path.to_string_lossy().into_owned(),
        })
    })
}

#[tauri::command]
fn open_exported_document(file_path: String, state: State<'_, AppState>) -> Result<(), String> {
    let (root, candidate) = with_store(&state, |store| {
        Ok((store.root().join("exports").join("reports"), PathBuf::from(&file_path)))
    })?;
    let root = fs::canonicalize(root).map_err(|error| error.to_string())?;
    let candidate = fs::canonicalize(candidate).map_err(|error| error.to_string())?;
    if !candidate.starts_with(&root)
        || candidate.is_symlink()
        || !candidate.is_file()
        || !matches!(candidate.extension().and_then(|value| value.to_str()), Some("html" | "md"))
    {
        return Err("Continuum only opens verified HTML or Markdown files from this project's report export folder.".into());
    }
    #[cfg(target_os = "linux")]
    {
        let mut child = std::process::Command::new("xdg-open")
            .arg(&candidate)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .map_err(|error| format!("Could not open the exported report: {error}"))?;
        std::thread::spawn(move || { let _ = child.wait(); });
        Ok(())
    }
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::ffi::OsStrExt;
        use windows_sys::Win32::UI::Shell::ShellExecuteW;
        use windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
        let path: Vec<u16> = candidate.as_os_str().encode_wide().chain(Some(0)).collect();
        // ShellExecute opens the verified export with the user's default app.
        // No command shell interprets a project-controlled filename.
        let result = unsafe {
            ShellExecuteW(
                std::ptr::null_mut(),
                std::ptr::null(),
                path.as_ptr(),
                std::ptr::null(),
                std::ptr::null(),
                SW_SHOWNORMAL,
            )
        };
        if result as isize <= 32 {
            Err(format!("Windows could not open the exported report (ShellExecute code {}).", result as isize))
        } else {
            Ok(())
        }
    }
    #[cfg(not(any(target_os = "linux", target_os = "windows")))]
    Err("Automatic report opening is not yet available on this operating system.".into())
}

#[tauri::command]
fn capture_runtime_health() -> CaptureRuntimeHealth {
    #[cfg(target_os = "linux")]
    let missing_recording_elements = recording_runtime::missing_elements();
    #[cfg(not(target_os = "linux"))]
    let missing_recording_elements: Vec<String> = Vec::new();
    #[cfg(target_os = "linux")]
    let missing_native_screen_elements = recording_runtime::missing_native_screen_elements();
    #[cfg(not(target_os = "linux"))]
    let missing_native_screen_elements: Vec<String> = Vec::new();
    #[cfg(target_os = "linux")]
    let missing_native_screen_audio_elements = recording_runtime::missing_native_screen_audio_elements();
    #[cfg(not(target_os = "linux"))]
    let missing_native_screen_audio_elements: Vec<String> = Vec::new();
    let recording_backend_available = if cfg!(target_os = "linux") {
        Some(missing_recording_elements.is_empty())
    } else { None };
    let scanner_ready = std::env::var_os("GST_PLUGIN_SCANNER_1_0")
        .map(PathBuf::from)
        .is_some_and(|path| path.is_file())
        || ["/usr/libexec/gstreamer-1.0/gst-plugin-scanner", "/usr/lib/x86_64-linux-gnu/gstreamer1.0/gstreamer-1.0/gst-plugin-scanner"]
            .iter().any(|path| Path::new(path).is_file());
    #[cfg(unix)]
    let pipewire_socket_ready = std::env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .and_then(|path| fs::metadata(path.join("pipewire-0")).ok())
        .is_some_and(|metadata| {
            use std::os::unix::fs::FileTypeExt;
            metadata.file_type().is_socket()
        });
    #[cfg(not(unix))]
    let pipewire_socket_ready = false;
    let desktop_portal_session_ready = std::env::var_os("DBUS_SESSION_BUS_ADDRESS").is_some();
    let native_screen_available = cfg!(target_os = "linux")
        && missing_native_screen_elements.is_empty()
        && pipewire_socket_ready && desktop_portal_session_ready;
    let native_screen_audio_available = native_screen_available
        && missing_native_screen_audio_elements.is_empty();
    let summary = if cfg!(target_os = "windows") {
        "Windows screen recording uses the system sharing dialog. System audio records the default output device through WASAPI; microphone access is requested separately. Each source is started only after your capture consent.".into()
    } else if native_screen_available {
        if native_screen_audio_available {
            "Screen and selected audio sources use one native Linux recorder. Choose a non-sensitive window in the system chooser.".into()
        } else {
            format!("Screen recording is ready, but combined audio needs these components: {}.", missing_native_screen_audio_elements.join(", "))
        }
    } else if !missing_recording_elements.is_empty() {
        format!("Recording components could not load: {}. This is a recording-runtime problem, not a permission denial. Use the updated Continuum package.", missing_recording_elements.join(", "))
    } else if scanner_ready && pipewire_socket_ready && desktop_portal_session_ready {
        "Recording and playback components loaded successfully. PipeWire and the portal session are present. Device permission and actual encoded output are checked separately when you start capture.".into()
    } else {
        format!(
            "Capture runtime needs attention: scanner={}, PipeWire={}, desktop portal session={}.",
            scanner_ready, pipewire_socket_ready, desktop_portal_session_ready
        )
    };
    CaptureRuntimeHealth {
        platform: std::env::consts::OS,
        recording_backend_available,
        missing_recording_elements,
        native_screen_available,
        missing_native_screen_elements,
        native_screen_audio_available,
        missing_native_screen_audio_elements,
        native_system_audio_available: system_audio::available(),
        scanner_ready,
        pipewire_socket_ready,
        desktop_portal_session_ready,
        summary,
    }
}

#[tauri::command]
fn create_capture_session(
    request: NewCaptureSession,
    state: State<'_, AppState>,
) -> Result<CaptureSession, String> {
    with_store(&state, |store| {
        store.create_capture_session(&desktop_user_command(), request)
    })
}

#[tauri::command]
fn get_capture_session(
    capture_session_id: String,
    state: State<'_, AppState>,
) -> Result<CaptureSession, String> {
    with_store(&state, |store| {
        store.get_capture_session(&capture_session_id)
    })
}

#[tauri::command]
fn list_capture_sessions(
    limit: u32,
    offset: u64,
    state: State<'_, AppState>,
) -> Result<CaptureSessionPage, String> {
    with_store(&state, |store| {
        store.list_capture_sessions(PageRequest { limit, offset })
    })
}

#[tauri::command]
fn list_capture_segments(
    capture_session_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<CaptureSegment>, String> {
    with_store(&state, |store| {
        store.list_capture_segments(
            &capture_session_id,
            PageRequest {
                limit: 100,
                offset: 0,
            },
        )
    })
}

#[tauri::command]
fn list_capture_markers(
    capture_session_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<CaptureMarker>, String> {
    with_store(&state, |store| {
        store.list_capture_markers(
            &capture_session_id,
            PageRequest {
                limit: 100,
                offset: 0,
            },
        )
    })
}

#[tauri::command]
fn get_capture_preview(
    artifact_id: String,
    state: State<'_, AppState>,
) -> Result<CapturePreview, String> {
    with_store(&state, |store| {
        let artifact = store.get_artifact(&artifact_id)?;
        let bytes = store.read_artifact_bounded(&artifact_id, 8 * 1024 * 1024)?;
        Ok(CapturePreview {
            artifact_id,
            media_type: artifact.media_type,
            bytes,
        })
    })
}

#[tauri::command]
fn record_capture_permission(
    capture_session_id: String,
    source_kind: CaptureSourceKind,
    status: CapturePermissionStatus,
    permission_reference: Option<String>,
    expected_version: i64,
    state: State<'_, AppState>,
) -> Result<CaptureSession, String> {
    with_store(&state, |store| {
        store.record_capture_permission(
            &desktop_user_command(),
            &capture_session_id,
            source_kind,
            status,
            permission_reference.as_deref(),
            expected_version,
        )
    })
}

#[tauri::command]
fn begin_capture(
    capture_session_id: String,
    indicator_id: String,
    expected_version: i64,
    state: State<'_, AppState>,
) -> Result<CaptureSession, String> {
    with_store(&state, |store| {
        store.begin_capture(
            &desktop_user_command(),
            &capture_session_id,
            &indicator_id,
            expected_version,
        )
    })
}

#[tauri::command]
fn pause_capture(
    capture_session_id: String,
    expected_version: i64,
    state: State<'_, AppState>,
) -> Result<CaptureSession, String> {
    with_store(&state, |store| {
        store.pause_capture(
            &desktop_user_command(),
            &capture_session_id,
            expected_version,
        )
    })
}

#[tauri::command]
fn resume_capture(
    capture_session_id: String,
    indicator_id: String,
    expected_version: i64,
    state: State<'_, AppState>,
) -> Result<CaptureSession, String> {
    with_store(&state, |store| {
        store.resume_capture(
            &desktop_user_command(),
            &capture_session_id,
            &indicator_id,
            expected_version,
        )
    })
}

#[tauri::command]
fn stop_capture(
    capture_session_id: String,
    expected_version: i64,
    state: State<'_, AppState>,
) -> Result<CaptureSession, String> {
    with_store(&state, |store| {
        store.stop_capture(
            &desktop_user_command(),
            &capture_session_id,
            expected_version,
        )
    })
}

#[tauri::command]
fn interrupt_capture(
    capture_session_id: String,
    reason: String,
    expected_version: i64,
    state: State<'_, AppState>,
) -> Result<CaptureSession, String> {
    with_store(&state, |store| {
        store.interrupt_capture(
            &desktop_user_command(),
            &capture_session_id,
            &reason,
            expected_version,
        )
    })
}

#[tauri::command]
fn fail_capture(
    capture_session_id: String,
    failure_code: String,
    failure_message: String,
    recoverable: bool,
    expected_version: i64,
    state: State<'_, AppState>,
) -> Result<CaptureSession, String> {
    with_store(&state, |store| {
        store.fail_capture(
            &desktop_user_command(),
            &capture_session_id,
            &failure_code,
            &failure_message,
            recoverable,
            expected_version,
        )
    })
}

#[tauri::command]
fn ingest_capture_segment(
    capture_session_id: String,
    segment: NewCaptureSegment,
    bytes: Vec<u8>,
    classification: ArtifactClassification,
    state: State<'_, AppState>,
) -> Result<CaptureSegment, String> {
    with_store(&state, |store| {
        store.ingest_capture_segment(
            &desktop_user_command(),
            &capture_session_id,
            segment,
            &bytes,
            classification,
        )
    })
}

#[tauri::command]
fn add_capture_marker(
    capture_session_id: String,
    marker: NewCaptureMarker,
    state: State<'_, AppState>,
) -> Result<CaptureMarker, String> {
    with_store(&state, |store| {
        store.add_capture_marker(&desktop_user_command(), &capture_session_id, marker)
    })
}

#[tauri::command]
fn promote_capture_recording(capture_session_id: String, title: String, annotation: String, state: State<'_, AppState>) -> Result<ResearchItem, String> {
    with_store(&state, |store| store.promote_capture_recording_to_evidence(&desktop_user_command(), &capture_session_id, &title, &annotation))
}

#[tauri::command]
fn promote_capture_segment_to_evidence(
    request: NewCaptureSegmentEvidence,
    state: State<'_, AppState>,
) -> Result<ResearchItem, String> {
    with_store(&state, |store| {
        store.promote_capture_segment_to_evidence(&desktop_user_command(), request)
    })
}

#[tauri::command]
fn promote_capture_marker_to_evidence(
    marker_id: String,
    title: String,
    summary: String,
    relevance: String,
    question_id: Option<String>,
    state: State<'_, AppState>,
) -> Result<ResearchItem, String> {
    with_store(&state, |store| {
        store.promote_capture_marker_to_evidence(
            &desktop_user_command(),
            &marker_id,
            &title,
            &summary,
            &relevance,
            question_id,
        )
    })
}

#[tauri::command]
fn capture_external_evidence(
    evidence: CaptureExternalEvidence,
    state: State<'_, AppState>,
) -> Result<ResearchItem, String> {
    with_store(&state, |store| {
        store.capture_external_evidence(&desktop_user_command(), evidence)
    })
}

#[cfg(target_os = "linux")]
#[tauri::command]
fn start_native_screen_capture(
    system_audio: bool,
    microphone: bool,
    consent: bool,
    state: State<'_, AppState>,
    screen: State<'_, native_screen::NativeScreenState>,
) -> Result<(), String> {
    let enabled = with_store(&state, |store| store.capability_enabled(Space::Research))?;
    if !enabled { return Err("Enable Research Space before recording the screen.".into()); }
    if !consent { return Err("Confirm permission to record the selected screen and audio sources.".into()); }
    if (system_audio || microphone) && !recording_runtime::missing_native_screen_audio_elements().is_empty() {
        return Err("Screen audio recording components are not installed in this build.".into());
    }
    screen.start(system_audio, microphone)
}

#[cfg(target_os = "linux")]
#[tauri::command]
fn cancel_native_screen_capture(screen: State<'_, native_screen::NativeScreenState>) {
    screen.cancel();
}

#[cfg(not(target_os = "linux"))]
#[tauri::command]
fn cancel_native_screen_capture() {}

#[cfg(not(target_os = "linux"))]
#[tauri::command]
fn start_native_screen_capture() -> Result<(), String> {
    Err("Native screen recording is currently supported on Linux only.".into())
}

#[cfg(target_os = "linux")]
#[tauri::command]
fn stop_native_screen_capture(
    title: String,
    annotation: String,
    research_session_id: Option<String>,
    state: State<'_, AppState>,
    screen: State<'_, native_screen::NativeScreenState>,
) -> Result<ResearchItem, String> {
    native_screen::trace_capture("save command entered");
    screen.finish()?;
    native_screen::trace_capture("native file finalized");
    let recorded = screen.take_ready()?.ok_or("No completed screen recording is awaiting save")?;
    let name = if title.trim().is_empty() { "Screen recording" } else { title.trim() };
    let input = CaptureExternalEvidence {
        kind: continuum_core::CaptureExternalKind::File,
        title: name.to_string(),
        source_uri: None,
        source_title: Some(name.to_string()),
        captured_at: None,
        media_type: Some(if recorded.system_audio || recorded.microphone {
            "video/webm; codecs=vp8,opus".into()
        } else { "video/webm; codecs=vp8".into() }),
        bytes: Some(recorded.bytes.clone()),
        source_content: None,
        annotation,
        summary: "User-approved screen recording captured through the Linux desktop portal with the selected audio sources.".into(),
        relevance: "Research or development work recorded by the user.".into(),
        research_session_id,
        research_question_id: None,
        capture_session_id: None,
        classification: ArtifactClassification::Internal,
        metadata: serde_json::json!({
            "backend": "linux-pipewire-portal",
            "duration_ms": recorded.duration_ms,
            "audio": recorded.system_audio || recorded.microphone,
            "system_audio": recorded.system_audio,
            "microphone": recorded.microphone,
            "explicit_user_action": true,
        }),
    };
    native_screen::trace_capture("saving evidence to project");
    match with_store(&state, |store| store.capture_external_evidence(&desktop_user_command(), input)) {
        Ok(evidence) => {
            native_screen::trace_capture("evidence saved");
            Ok(evidence)
        },
        Err(error) => {
            screen.restore_ready(recorded)?;
            Err(format!("Recording is held in memory but could not be added to the workspace: {error}. Press Stop & save again to retry."))
        }
    }
}

#[cfg(not(target_os = "linux"))]
#[tauri::command]
fn stop_native_screen_capture(
    _title: String,
    _annotation: String,
    _research_session_id: Option<String>,
) -> Result<ResearchItem, String> {
    Err("Native screen recording is currently supported on Linux only.".into())
}

#[tauri::command]
fn list_active_research_sessions(state: State<'_, AppState>) -> Result<ResearchSearchPage, String> {
    with_store(&state, |store| {
        store.search_research(ResearchSearchQuery {
            text: String::new(),
            entity_type: Some(ResearchEntityKind::ResearchSession),
            status: Some("active".into()),
            session_id: None,
            page: PageRequest {
                limit: 100,
                offset: 0,
            },
        })
    })
}

#[tauri::command]
fn research_space_enabled(state: State<'_, AppState>) -> Result<bool, String> {
    with_store(&state, |store| {
        store.capability_enabled(continuum_core::Space::Research)
    })
}

#[tauri::command]
fn space_capabilities(state: State<'_, AppState>) -> Result<SpaceCapabilities, String> {
    with_store(&state, |store| {
        Ok(SpaceCapabilities {
            research: store.capability_enabled(Space::Research)?,
            development: store.capability_enabled(Space::Development)?,
        })
    })
}

#[tauri::command]
fn current_project_state(
    scope: CheckpointScope,
    checkpoint_id: Option<String>,
    state: State<'_, AppState>,
) -> Result<CurrentProjectState, String> {
    with_store(&state, |store| {
        store.current_project_state(CurrentProjectStateRequest {
            scope,
            checkpoint_id,
        })
    })
}

#[tauri::command]
fn list_context_checkpoints(
    scope: Option<CheckpointScope>,
    state: State<'_, AppState>,
) -> Result<CheckpointEnvelopePage, String> {
    with_store(&state, |store| {
        store.list_checkpoint_envelopes(
            scope,
            PageRequest {
                limit: 100,
                offset: 0,
            },
        )
    })
}

#[tauri::command]
fn create_context_checkpoint(
    input: SemanticCheckpointInput,
    state: State<'_, AppState>,
) -> Result<CheckpointEnvelope, String> {
    with_store(&state, |store| {
        store.create_semantic_checkpoint(&desktop_user_command(), input)
    })
}

#[tauri::command]
fn compare_context_checkpoints(
    first_checkpoint_id: String,
    second_checkpoint_id: String,
    state: State<'_, AppState>,
) -> Result<CheckpointComparison, String> {
    with_store(&state, |store| {
        store.compare_checkpoints(&first_checkpoint_id, &second_checkpoint_id)
    })
}

#[tauri::command]
fn build_context_pack(
    request: ContextPackRequest,
    state: State<'_, AppState>,
) -> Result<ContextPack, String> {
    with_store(&state, |store| store.build_context_pack(request))
}

#[tauri::command]
fn save_context_pack(
    pack: ContextPack,
    state: State<'_, AppState>,
) -> Result<SavedContextPack, String> {
    with_store(&state, |store| {
        store.save_context_pack(&desktop_user_command(), &pack)
    })
}

#[tauri::command]
fn get_saved_context_pack(
    context_pack_id: String,
    state: State<'_, AppState>,
) -> Result<SavedContextPack, String> {
    with_store(&state, |store| {
        store.get_saved_context_pack(&context_pack_id)
    })
}

#[tauri::command]
fn list_saved_context_packs(state: State<'_, AppState>) -> Result<SavedContextPackPage, String> {
    with_store(&state, |store| {
        store.list_saved_context_packs(PageRequest {
            limit: 100,
            offset: 0,
        })
    })
}

#[tauri::command]
fn create_mcp_client_grant(
    input: NewMcpClientGrant,
    state: State<'_, AppState>,
) -> Result<CreatedMcpGrant, String> {
    with_store(&state, |store| {
        store.create_mcp_client_grant(&desktop_user_command(), input)
    })
}

#[tauri::command]
fn list_mcp_client_grants(state: State<'_, AppState>) -> Result<McpGrantPage, String> {
    with_store(&state, |store| {
        store.list_mcp_client_grants(PageRequest {
            limit: 100,
            offset: 0,
        })
    })
}

#[tauri::command]
fn revoke_mcp_client_grant(
    grant_id: String,
    state: State<'_, AppState>,
) -> Result<McpClientGrant, String> {
    with_store(&state, |store| {
        store.revoke_mcp_client_grant(&desktop_user_command(), &grant_id)
    })
}

#[tauri::command]
fn list_external_proposals(
    status: Option<String>,
    state: State<'_, AppState>,
) -> Result<ExternalProposalPage, String> {
    with_store(&state, |store| {
        store.list_external_proposals(
            status.as_deref(),
            PageRequest {
                limit: 100,
                offset: 0,
            },
        )
    })
}

#[tauri::command]
fn get_external_proposal(
    proposal_id: String,
    state: State<'_, AppState>,
) -> Result<ExternalProposal, String> {
    with_store(&state, |store| store.get_external_proposal(&proposal_id))
}

#[tauri::command]
fn review_external_proposal(
    proposal_id: String,
    expected_version: i64,
    decision: ProposalReviewDecision,
    review_note: String,
    state: State<'_, AppState>,
) -> Result<ExternalProposal, String> {
    with_store(&state, |store| {
        store.review_external_proposal(
            &desktop_user_command(),
            &proposal_id,
            expected_version,
            decision,
            &review_note,
        )
    })
}

#[tauri::command]
fn list_mcp_audit(state: State<'_, AppState>) -> Result<McpAuditPage, String> {
    with_store(&state, |store| {
        store.list_mcp_audit(PageRequest {
            limit: 100,
            offset: 0,
        })
    })
}

fn main() {
    configure_gstreamer_plugins();
    let builder = tauri::Builder::default()
        .setup(|app| {
            #[cfg(any(target_os = "linux", windows))]
            media_permissions::install(app)?;
            Ok(())
        })
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .manage(AppState::default())
        .manage(system_audio::SystemAudioState::default())
        .manage(github_cli::LoginState::default());
    #[cfg(target_os = "linux")]
    let builder = builder.manage(native_screen::NativeScreenState::default());
    builder
        .on_window_event(|window,event| {
            if matches!(event,tauri::WindowEvent::Destroyed) {
                use tauri::Manager;
                window.state::<system_audio::SystemAudioState>().stop();
                #[cfg(target_os = "linux")]
                window.state::<native_screen::NativeScreenState>().cancel();
            }
        })
        .manage(workspace::DockState::default())
        .invoke_handler(tauri::generate_handler![
            system_audio::start_system_audio,
            system_audio::acknowledge_system_audio,
            system_audio::stop_system_audio,
            start_native_screen_capture,
            cancel_native_screen_capture,
            stop_native_screen_capture,
            assistant::load_workspace_document,
            assistant::recent_workspace_ai,
            assistant::list_mcp_report_drafts,
            assistant::save_workspace_document,
            assistant::load_workspace_revision,
            assistant::inspect_workspace_memory,
            assistant::run_workspace_assistant,
            assistant::workspace_media,
            assistant::export_workspace_markdown,
            workspace::start_research_workspace,
            workspace::workspace_research_items,
            workspace::archive_workspace_evidence,
            workspace::annotate_workspace_evidence,
            workspace::update_workspace_evidence,
            workspace::update_workspace_research_session,
            workspace::update_workspace_research_question,
            workspace::detect_ai_clients,
            workspace::workspace_mcp_server,
            workspace::install_ai_client_connection,
            workspace::inspect_ai_client_connection,
            workspace::remove_ai_client_connection,
            workspace::run_ai_organizer,
            workspace::materialize_board_proposal,
            workspace::workspace_thumbnail,
            workspace::export_workspace_context,
            workspace::set_capture_dock,
            open_project,
            create_project,
            close_project,
            run_release_diagnostics,
            backup_project,
            export_project,
            restore_project,
            project_remote_status,
            github_cli::github_status,
            github_cli::github_begin_login,
            github_cli::github_login_progress,
            github_cli::github_open_authorization_page,
            github_cli::github_list_repositories,
            github_cli::github_list_branches,
            github_cli::github_open_new_repository,
            publish_project_snapshot,
            restore_project_from_remote,
            create_workspace_question,
            attach_and_sync_repository,
            compose_human_document,
            save_human_document,
            list_human_documents,
            publish_human_document,
            open_exported_document,
            capture_runtime_health,
            create_capture_session,
            get_capture_session,
            list_capture_sessions,
            list_capture_segments,
            list_capture_markers,
            get_capture_preview,
            record_capture_permission,
            begin_capture,
            pause_capture,
            resume_capture,
            stop_capture,
            interrupt_capture,
            fail_capture,
            ingest_capture_segment,
            add_capture_marker,
            promote_capture_segment_to_evidence,
            promote_capture_recording,
            promote_capture_marker_to_evidence,
            capture_external_evidence,
            list_active_research_sessions,
            research_space_enabled,
            space_capabilities,
            current_project_state,
            list_context_checkpoints,
            create_context_checkpoint,
            compare_context_checkpoints,
            build_context_pack,
            save_context_pack,
            get_saved_context_pack,
            list_saved_context_packs,
            create_mcp_client_grant,
            list_mcp_client_grants,
            revoke_mcp_client_grant,
            list_external_proposals,
            get_external_proposal,
            review_external_proposal,
            list_mcp_audit
        ])
        .run(tauri::generate_context!())
        .expect("error while running Continuum");
}

fn configure_gstreamer_plugins() {
    let Ok(executable) = std::env::current_exe() else {
        return;
    };
    let Some(prefix) = executable.parent().and_then(Path::parent) else {
        return;
    };
    let plugin_directory = prefix.join("lib").join("Continuum").join("gstreamer-1.0");
    if !plugin_directory.is_dir() {
        return;
    }
    let mut paths = vec![plugin_directory.clone()];
    if let Some(existing) = std::env::var_os("GST_PLUGIN_PATH_1_0") {
        paths.extend(std::env::split_paths(&existing));
    }
    if let Ok(value) = std::env::join_paths(paths) {
        // This runs before Tauri/WebKit starts any threads or child processes.
        unsafe { std::env::set_var("GST_PLUGIN_PATH_1_0", value) };
    }
    let scanner = plugin_directory.join("gst-plugin-scanner");
    if scanner.is_file() {
        // GStreamer otherwise searches a host-specific libexec path that does not exist
        // inside an AppImage mount.
        unsafe {
            std::env::set_var("GST_PLUGIN_SCANNER_1_0", &scanner);
            std::env::set_var("GST_PLUGIN_SCANNER", &scanner);
        }
    }
}
