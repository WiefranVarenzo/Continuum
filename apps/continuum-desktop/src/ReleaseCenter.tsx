import type { OpenedProject, ReleaseDiagnostics } from "./releaseContracts";
import type { ProjectPublishResult } from "./releaseContracts";
import { ProjectRemotePanel } from "./ProjectRemotePanel";

interface Props {
  project: OpenedProject;
  diagnostics: ReleaseDiagnostics | null;
  busy: boolean;
  onDiagnose: () => Promise<void>;
  onBackup: () => Promise<void>;
  onExport: () => Promise<void>;
  onPublishRemote?: (remoteUrl: string, branch: string | null, privateConfirmed: boolean) => Promise<ProjectPublishResult>;
  onClose: () => Promise<void>;
}

export function ReleaseCenter({ project, diagnostics, busy, onDiagnose, onBackup, onExport, onPublishRemote, onClose }: Props) {
  return <section className="release-center" aria-labelledby="release-center-title">
    <div className="release-heading">
      <div><p className="eyebrow">Active project</p><h2 id="release-center-title">{project.name}</h2><p title={project.path}>{project.path}</p></div>
      <span className={`health-badge ${project.integrity_healthy ? "healthy" : "attention"}`}>{project.integrity_healthy ? "Healthy" : "Needs attention"}</span>
    </div>
    <div className="project-facts">
      <span>Ledger <strong>{project.ledger_sequence}</strong></span>
      <span>{project.research ? "Research on" : "Research off"}</span>
      <span>{project.development ? "Development on" : "Development off"}</span>
    </div>
    <div className="release-actions">
      <button disabled={busy} onClick={() => void onDiagnose()}>Run health check</button>
      <button disabled={busy} onClick={() => void onBackup()}>Backup database</button>
      <button disabled={busy} onClick={() => void onExport()}>Export restorable copy</button>
      <button disabled={busy} onClick={() => void onClose()}>Close project</button>
    </div>
    {diagnostics && <div className={`diagnostic-result ${diagnostics.integrity_healthy ? "healthy" : "attention"}`} aria-live="polite">
      <strong>{diagnostics.integrity_healthy ? "All integrity checks passed" : `${diagnostics.issues.length} issue(s) need attention`}</strong>
      <span>Continuum {diagnostics.app_version} · schema {diagnostics.schema_version} · {diagnostics.checked_artifacts} artifact(s) checked</span>
      {diagnostics.issues.length > 0 && <ul>{diagnostics.issues.map((issue) => <li key={`${issue.code}-${issue.path_or_id}`}><strong>{issue.code}</strong>: {issue.guidance}</li>)}</ul>}
    </div>}
    {onPublishRemote && <ProjectRemotePanel projectId={project.project_id} busy={busy} onPublish={onPublishRemote} />}
  </section>;
}
