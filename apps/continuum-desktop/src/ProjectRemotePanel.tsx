import { useEffect, useState, type FormEvent } from "react";
import { getProjectRemoteStatus, isDesktopRuntime } from "./bridge";
import type { ProjectPublishResult, ProjectRemoteStatus } from "./releaseContracts";
import { GithubRepositoryPicker } from "./GithubRepositoryPicker";

interface Props {
  projectId: string;
  busy: boolean;
  onPublish: (remoteUrl: string, branch: string | null, privateConfirmed: boolean) => Promise<ProjectPublishResult>;
}

export function ProjectRemotePanel({ projectId, busy, onPublish }: Props) {
  const [status, setStatus] = useState<ProjectRemoteStatus | null>(null);
  const [remoteUrl, setRemoteUrl] = useState("");
  const [branch, setBranch] = useState("");
  const [confirmed, setConfirmed] = useState(false);
  const [working, setWorking] = useState(false);
  const [uploadSeconds, setUploadSeconds] = useState(0);
  const [message, setMessage] = useState("");

  useEffect(() => {
    if (!working) return;
    setUploadSeconds(0);
    const timer = window.setInterval(() => setUploadSeconds((seconds) => seconds + 1), 1000);
    return () => window.clearInterval(timer);
  }, [working]);

  useEffect(() => {
    if (!isDesktopRuntime()) return;
    let active = true;
    void getProjectRemoteStatus().then((next) => {
      if (!active) return;
      setStatus(next);
      setRemoteUrl(next.remote_url ?? "");
      setBranch(next.branch ?? "");
    }).catch((cause) => { if (active) setMessage(String(cause)); });
    return () => { active = false; };
  }, [projectId]);

  async function submit(event: FormEvent) {
    event.preventDefault();
    setWorking(true);
    setMessage("");
    try {
      const result = await onPublish(remoteUrl.trim(), branch.trim() || null, confirmed);
      setStatus(await getProjectRemoteStatus());
      setMessage(result.changed ? `Uploaded local snapshot ${result.commit_id.slice(0, 10)}.` : "The remote already has this snapshot.");
      setConfirmed(false);
    } catch (cause) {
      setMessage(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setWorking(false);
    }
  }

  return <section className="project-remote-panel" aria-labelledby="project-remote-title">
    <div><p className="eyebrow">Optional · GitHub / GitLab</p><h3 id="project-remote-title">Save a version of this project</h3><p>The project on this computer remains the working copy. Upload uses a verified snapshot; it never imports remote changes into your open project.</p></div>
    {status && (!status.git_available || !status.lfs_available) && <p className="remote-warning" role="status">{!status.git_available ? "Install Git first." : "Install Git LFS first (on Fedora: sudo dnf install git-lfs)."} No project data was uploaded.</p>}
    <GithubRepositoryPicker remoteUrl={remoteUrl} branch={branch} disabled={busy || working || Boolean(status?.remote_url)} onSelect={(url, selectedBranch) => { setRemoteUrl(url); setBranch(selectedBranch); }} />
    <form onSubmit={(event) => void submit(event)}>
      <label>Private repository URL<input value={remoteUrl} onChange={(event) => { setRemoteUrl(event.target.value); setBranch(""); }} readOnly={Boolean(status?.remote_url)} placeholder="https://github.com/you/continuum-project.git" required /></label>
      {branch && <small>Selected branch: {branch}</small>}
      <small>{status?.remote_url ? `Linked to ${status.branch ?? "default branch"} · last upload ${status.last_head?.slice(0, 10)}` : "First upload needs an empty destination branch. Existing remote work is never overwritten."}</small>
      {status && <small>Current database + media: approximately {(status.estimated_current_bytes / (1024 * 1024)).toFixed(1)} MiB. Earlier LFS versions and downloads may use more quota.</small>}
      <label className="remote-confirm"><input type="checkbox" checked={confirmed} onChange={(event) => setConfirmed(event.target.checked)} />I confirm this repository is private and I have reviewed the project evidence for sensitive content.</label>
      <button className="primary-action" disabled={busy || working || !status?.git_available || !status?.lfs_available || !remoteUrl.trim() || !confirmed}>{working ? "Uploading…" : "Save local version & upload"}</button>
      {working && <p role="status">Uploading the local snapshot and Git LFS media… {uploadSeconds}s elapsed. Larger recordings can take several minutes; keep Continuum open.</p>}
    </form>
    {message && <p className="remote-warning" role="status">{message}</p>}
    <p className="remote-footnote">A remote conflict stops the upload; local cards, recordings, messages, reports, and memory are never replaced. Importing a remote project is a separate action on Home.</p>
  </section>;
}
