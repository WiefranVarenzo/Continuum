import { useState } from "react";
import { HumanDocumentView } from "./HumanDocumentView";
import { CapturePanel } from "./CapturePanel";
import { ContinuityPanel } from "./ContinuityPanel";
import { AiContinuityPanel } from "./AiContinuityPanel";
import { ProjectLauncher } from "./ProjectLauncher";
import { ReleaseCenter } from "./ReleaseCenter";
import { WorkspaceQuickStart } from "./WorkspaceQuickStart";
import { Workspace } from "./Workspace";
import { sampleDocument } from "./sample";
import type { HumanDocument } from "./contracts";
import {
  backupProject,
  attachAndSyncRepository,
  chooseDirectory,
  closeProject,
  composeCurrentDocument,
  createProject,
  createWorkspaceQuestion,
  exportProject,
  getSpaceCapabilities,
  isDesktopRuntime,
  openProject,
  openExportedDocument,
  publishDocument,
  publishProjectSnapshot,
  restoreProject,
  restoreProjectFromRemote,
  runReleaseDiagnostics,
  saveDocument,
} from "./bridge";
import type { SpaceCapabilities } from "./contextContracts";
import type { CreateProjectInput, OpenedProject, ProjectPublishResult, ReleaseDiagnostics, RepositorySyncSummary, RestoreProjectInput, RestoreRemoteInput, WorkspaceQuestionInput } from "./releaseContracts";
import "./cp8-controls.css";
import "./release.css";
import "./project-remote.css";

const LAST_PROJECT_KEY = "continuum.last-project.v1";
const RECENT_PROJECTS_KEY = "continuum.recent-projects.v1";
interface RecentProject { path: string; name: string; research: boolean; development: boolean; openedAt: string }
function recentProjects(): RecentProject[] {
  try {
    const saved = JSON.parse(localStorage.getItem(RECENT_PROJECTS_KEY) || "[]");
    if (Array.isArray(saved) && saved.length) return saved.filter((item): item is RecentProject => typeof item?.path === "string" && typeof item?.name === "string").slice(0, 12);
  } catch { /* A damaged shortcut list must never prevent project opening. */ }
  const path = localStorage.getItem(LAST_PROJECT_KEY);
  return path ? [{path,name:path.split(/[\\/]/).filter(Boolean).at(-1) || "Recent project",research:true,development:true,openedAt:""}] : [];
}

export default function App() {
  const [document, setDocument] = useState<HumanDocument>(sampleDocument);
  const [error, setError] = useState("");
  const [busy, setBusy] = useState(false);
  const [notice, setNotice] = useState("");
  const [project, setProject] = useState<OpenedProject | null>(null);
  const [diagnostics, setDiagnostics] = useState<ReleaseDiagnostics | null>(null);
  const [capabilities, setCapabilities] = useState<SpaceCapabilities>({ research: false, development: false });
  const [recent, setRecent] = useState<RecentProject[]>(recentProjects);
  const desktop = isDesktopRuntime();
  const projectOpen = project !== null;

  async function loadOpened(opened: OpenedProject, message: string) {
    const [nextDocument, nextCapabilities] = await Promise.all([composeCurrentDocument(), getSpaceCapabilities()]);
    setProject(opened);
    setDocument(nextDocument);
    setCapabilities(nextCapabilities);
    setDiagnostics(null);
    localStorage.setItem(LAST_PROJECT_KEY, opened.path);
    const nextRecent = [{path:opened.path,name:opened.name,research:opened.research,development:opened.development,openedAt:new Date().toISOString()},...recentProjects().filter(item=>item.path!==opened.path)].slice(0,12);
    localStorage.setItem(RECENT_PROJECTS_KEY,JSON.stringify(nextRecent));
    setRecent(nextRecent);
    setNotice(message);
    setError("");
  }

  async function open(path: string, quiet = false) {
    if (!path.trim()) return;
    setBusy(true);
    try {
      const opened = await openProject(path.trim());
      await loadOpened(opened, quiet ? "" : "Project opened and its current report was composed.");
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
      if (quiet) localStorage.removeItem(LAST_PROJECT_KEY);
    } finally {
      setBusy(false);
    }
  }

  async function create(input: CreateProjectInput) {
    setBusy(true);
    try {
      await loadOpened(await createProject(input), "Project created, verified, and ready to use.");
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setBusy(false);
    }
  }

  async function restore(input: RestoreProjectInput) {
    setBusy(true);
    try {
      await loadOpened(await restoreProject(input), "Verified export restored and opened.");
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setBusy(false);
    }
  }

  async function restoreRemote(input: RestoreRemoteInput) {
    setBusy(true);
    try {
      await loadOpened(await restoreProjectFromRemote(input), "Project downloaded, media verified, and restored into a new local folder.");
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
      throw cause;
    } finally {
      setBusy(false);
    }
  }

  async function publishRemote(remoteUrl: string, branch: string | null, privateConfirmed: boolean): Promise<ProjectPublishResult> {
    setBusy(true);
    try {
      await (await import("./projectDocuments")).prepareWorkspaceClose();
      const result = await publishProjectSnapshot(remoteUrl, branch, privateConfirmed);
      setNotice(result.changed ? `Local project snapshot uploaded to ${result.remote_url}. Commit ${result.commit_id.slice(0, 10)}.` : "Project snapshot already matches the connected repository.");
      setError("");
      return result;
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
      throw cause;
    } finally {
      setBusy(false);
    }
  }

  async function saveAndPublish(format: "html" | "markdown") {
    setBusy(true);
    try {
      const saved = await saveDocument(document);
      setDocument(saved);
      const published = await publishDocument(saved.id, format);
      if (format === "html") await openExportedDocument(published.file_path);
      setNotice(`${format.toUpperCase()} report saved to ${published.file_path}. Verified artifact: ${published.artifact_id}.`);
      setError("");
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setBusy(false);
    }
  }

  async function diagnose() {
    setBusy(true);
    try {
      const result = await runReleaseDiagnostics();
      setDiagnostics(result);
      setProject((current) => current ? { ...current, integrity_healthy: result.integrity_healthy, ledger_sequence: result.ledger_sequence } : current);
      setNotice(result.integrity_healthy ? "Project health check passed." : "Health check found issues; review the recovery guidance below.");
      setError("");
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setBusy(false);
    }
  }

  async function backup() {
    const destination = await chooseDirectory("Choose backup destination");
    if (!destination) return;
    setBusy(true);
    try {
      setNotice(`Verified database backup created at ${await backupProject(destination)}.`);
      setError("");
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setBusy(false);
    }
  }

  async function exportRestorable() {
    const destination = await chooseDirectory("Choose export destination");
    if (!destination) return;
    setBusy(true);
    try {
      setNotice(`Restorable project export created at ${await exportProject(destination)}.`);
      setError("");
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setBusy(false);
    }
  }

  async function close() {
    setBusy(true);
    try {
      await (await import("./projectDocuments")).prepareWorkspaceClose();
      await closeProject();
      localStorage.removeItem(LAST_PROJECT_KEY);
      setProject(null);
      setDiagnostics(null);
      setCapabilities({ research: false, development: false });
      setDocument(sampleDocument);
      setNotice("Project closed safely.");
      setError("");
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setBusy(false);
    }
  }

  async function refreshProject(message: string) {
    if (!project) return;
    await loadOpened(await openProject(project.path), message);
  }

  async function refreshWorkspace() {
    const nextDocument = await composeCurrentDocument();
    setDocument(nextDocument);
  }

  async function addQuestion(input: WorkspaceQuestionInput) {
    setBusy(true);
    try {
      await createWorkspaceQuestion(input);
      await refreshProject("Research question saved and the project report was refreshed.");
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
      throw cause;
    } finally {
      setBusy(false);
    }
  }

  async function syncRepository(path: string): Promise<RepositorySyncSummary> {
    setBusy(true);
    try {
      const result = await attachAndSyncRepository(path);
      await refreshProject("Repository synchronized, analyzed, and added to the report.");
      return result;
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
      throw cause;
    } finally {
      setBusy(false);
    }
  }

  return <div className="app-shell">
    {!(desktop && projectOpen) && <nav className="app-bar" aria-label="Report tools">
      <strong>CONTINUUM</strong>
      <div className="nav-actions">
        {!desktop && <span className="preview-badge">Design preview</span>}
        {desktop && projectOpen && <><button disabled={busy} onClick={() => saveAndPublish("html")}>Export HTML</button><button disabled={busy} onClick={() => saveAndPublish("markdown")}>Export Markdown</button></>}
      </div>
    </nav>}
    {error && <p className="error" role="alert">{error}</p>}
    {notice && <p className="notice global-notice" aria-live="polite">{notice}</p>}
    {desktop && project ? <Workspace key={project.project_id} project={project} capabilities={capabilities} document={document} busy={busy} onRefresh={refreshWorkspace} onExport={saveAndPublish} onRepository={syncRepository} onAllProjects={close} settings={<ReleaseCenter project={project} diagnostics={diagnostics} busy={busy} onDiagnose={diagnose} onBackup={backup} onExport={exportRestorable} onPublishRemote={publishRemote} onClose={close} />} /> : desktop && !projectOpen ? <ProjectLauncher busy={busy} recent={recent} onOpen={open} onCreate={create} onRestore={restore} onRestoreRemote={restoreRemote} onError={setError} /> : <>
      {project && <ReleaseCenter project={project} diagnostics={diagnostics} busy={busy} onDiagnose={diagnose} onBackup={backup} onExport={exportRestorable} onPublishRemote={publishRemote} onClose={close} />}
      {project && <WorkspaceQuickStart research={capabilities.research} development={capabilities.development} busy={busy} onQuestion={addQuestion} onRepository={syncRepository} onError={setError} />}
      <ContinuityPanel projectOpen={projectOpen} capabilities={capabilities} />
      <AiContinuityPanel projectOpen={projectOpen} capabilities={capabilities} />
      <CapturePanel projectOpen={projectOpen} researchEnabled={capabilities.research} />
      <HumanDocumentView document={document} />
    </>}
  </div>;
}
