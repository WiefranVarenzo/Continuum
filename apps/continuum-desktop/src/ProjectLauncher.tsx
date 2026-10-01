import { useState, type FormEvent } from "react";
import { chooseDirectory } from "./bridge";
import type { CreateProjectInput, RestoreProjectInput, RestoreRemoteInput } from "./releaseContracts";
import { GithubRepositoryPicker } from "./GithubRepositoryPicker";

interface Props {
  busy: boolean;
  recent: {path:string;name:string;research:boolean;development:boolean;openedAt:string}[];
  onOpen: (path: string) => Promise<void>;
  onCreate: (input: CreateProjectInput) => Promise<void>;
  onRestore: (input: RestoreProjectInput) => Promise<void>;
  onRestoreRemote?: (input: RestoreRemoteInput) => Promise<void>;
  onError: (message: string) => void;
}

function folderName(value: string): string {
  return value.trim().toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-|-$/g, "").slice(0, 72) || "continuum-project";
}

export function ProjectLauncher({ busy, recent, onOpen, onCreate, onRestore, onRestoreRemote, onError }: Props) {
  const [tab, setTab] = useState<"open" | "create" | "restore" | "from repo">("open");
  const [openPath, setOpenPath] = useState("");
  const [name, setName] = useState("");
  const [directoryName, setDirectoryName] = useState("");
  const [parentPath, setParentPath] = useState("");
  const [mode, setMode] = useState<"research" | "development" | "connected">("connected");
  const [sourcePath, setSourcePath] = useState("");
  const [restoreParent, setRestoreParent] = useState("");
  const [restoreName, setRestoreName] = useState("");
  const [remoteUrl, setRemoteUrl] = useState("");
  const [remoteBranch, setRemoteBranch] = useState("");
  const [remoteFeedback, setRemoteFeedback] = useState("");
  const [remoteWorking, setRemoteWorking] = useState(false);

  function chooseMode(nextMode: typeof mode) {
    setMode(nextMode);
    setTab("create");
  }

  async function pick(setter: (path: string) => void, title: string) {
    try {
      const selected = await chooseDirectory(title);
      if (selected) setter(selected);
    } catch (cause) {
      onError(cause instanceof Error ? cause.message : String(cause));
    }
  }

  async function submitCreate(event: FormEvent) {
    event.preventDefault();
    const safeDirectory = directoryName.trim() || folderName(name);
    await onCreate({
      parentPath,
      directoryName: safeDirectory,
      name,
      research: mode !== "development",
      development: mode !== "research",
    });
  }

  async function submitRemote(event: FormEvent) {
    event.preventDefault();
    if (!remoteUrl.trim()) { setRemoteFeedback("Choose a GitHub repository or enter its URL first."); return; }
    if (!restoreParent.trim()) { setRemoteFeedback("Choose a local parent folder under ‘Restore into’ first."); return; }
    if (!restoreName.trim()) { setRemoteFeedback("Enter a name for the new local project folder first."); return; }
    if (!onRestoreRemote) { setRemoteFeedback("Repository import is unavailable in this build."); return; }
    setRemoteFeedback("Downloading the project and verifying its media…");
    setRemoteWorking(true);
    try {
      await onRestoreRemote({ remoteUrl: remoteUrl.trim(), branch: remoteBranch || undefined, destinationParent: restoreParent.trim(), directoryName: restoreName.trim() });
      setRemoteFeedback("");
    } catch (cause) {
      setRemoteFeedback(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setRemoteWorking(false);
    }
  }

  return (
    <main className="welcome" aria-labelledby="welcome-title">
      <section className="welcome-hero">
        <p className="eyebrow">CONTINUUM · HOME</p>
        <h1 id="welcome-title">Welcome back to your work.</h1>
        <p>Open a recent project, continue research or development, or start something new. Project data stays in its own folder.</p>
        <div className="recent-projects" aria-label="Recent projects">
          <h2>Recent projects</h2>
          {recent.length ? recent.map(item=><button type="button" key={item.path} aria-label={`Open ${item.name}`} disabled={busy} onClick={()=>void onOpen(item.path)}><strong>{item.name}</strong><span>{item.research && item.development ? "Research & development" : item.research ? "Research" : "Development"}{item.openedAt ? ` · ${new Date(item.openedAt).toLocaleDateString()}` : ""}</span><small>{item.path}</small></button>) : <p>No recent projects yet. Open an existing folder or create a project.</p>}
        </div>
        <div className="mode-pills" aria-label="Create a workspace by mode">
          <button type="button" onClick={() => chooseMode("research")}>Research only</button>
          <button type="button" onClick={() => chooseMode("development")}>Development only</button>
          <button type="button" onClick={() => chooseMode("connected")}>Connected R&amp;D</button>
        </div>
      </section>

      <section className="launcher-card" aria-label="Project setup">
        <div className="launcher-tabs" role="tablist" aria-label="Project action">
          {(["open", "create", "restore", "from repo"] as const).map((value) => (
            <button key={value} type="button" role="tab" aria-selected={tab === value} onClick={() => setTab(value)}>{value}</button>
          ))}
        </div>

        {tab === "open" && <form className="launcher-form" onSubmit={(event) => { event.preventDefault(); void onOpen(openPath); }}>
          <div><h2>Open a Continuum project</h2><p>Select a folder containing <code>continuum.project.json</code>.</p></div>
          <label>Project folder<div className="path-picker"><input value={openPath} onChange={(event) => setOpenPath(event.target.value)} placeholder="Choose a project folder" /><button type="button" onClick={() => void pick(setOpenPath, "Open Continuum project")}>Browse…</button></div></label>
          <button className="primary-action" disabled={busy || !openPath.trim()}>{busy ? "Opening…" : "Open project"}</button>
        </form>}

        {tab === "create" && <form className="launcher-form" onSubmit={(event) => void submitCreate(event)}>
          <div><h2>Create a new project</h2><p>Your ledger, artifacts, checkpoints, reports, and AI access stay in this folder.</p></div>
          <label>Project name<input value={name} onChange={(event) => setName(event.target.value)} placeholder="My R&D project" maxLength={200} /></label>
          <label>Workspace mode<select value={mode} onChange={(event) => setMode(event.target.value as typeof mode)}><option value="connected">Connected R&amp;D</option><option value="research">Research only</option><option value="development">Development only</option></select></label>
          <label>Parent folder<div className="path-picker"><input value={parentPath} onChange={(event) => setParentPath(event.target.value)} placeholder="Where the project folder will be created" /><button type="button" onClick={() => void pick(setParentPath, "Choose parent folder")}>Browse…</button></div></label>
          <label>New folder name<input value={directoryName} onChange={(event) => setDirectoryName(event.target.value)} placeholder={name.trim() ? folderName(name) : "Choose a folder name"} maxLength={120} /><small>Optional. If empty, use the project name shown as a suggestion.</small></label>
          <button className="primary-action" disabled={busy || !name.trim() || !parentPath.trim()}>{busy ? "Creating…" : "Create and open"}</button>
        </form>}

        {tab === "restore" && <form className="launcher-form" onSubmit={(event) => { event.preventDefault(); void onRestore({ sourcePath, destinationParent: restoreParent, directoryName: restoreName }); }}>
          <div><h2>Restore a verified export</h2><p>The export is copied to a new folder and checked before it becomes active.</p></div>
          <label>Export folder<div className="path-picker"><input value={sourcePath} onChange={(event) => setSourcePath(event.target.value)} /><button type="button" onClick={() => void pick(setSourcePath, "Choose Continuum export")}>Browse…</button></div></label>
          <label>Restore into<div className="path-picker"><input value={restoreParent} onChange={(event) => setRestoreParent(event.target.value)} /><button type="button" onClick={() => void pick(setRestoreParent, "Choose restore destination")}>Browse…</button></div></label>
          <label>New folder name<input value={restoreName} onChange={(event) => setRestoreName(event.target.value)} placeholder="restored-project" maxLength={120} /></label>
          <button className="primary-action" disabled={busy || !sourcePath || !restoreParent || !restoreName}>{busy ? "Restoring…" : "Restore and verify"}</button>
        </form>}
        {tab === "from repo" && <form className="launcher-form" noValidate onSubmit={(event) => void submitRemote(event)}>
          <div><h2>Get a project from GitHub or GitLab</h2><p>Choose a repository previously uploaded from Continuum and a local destination. Continuum downloads the project and its media into a new folder. Existing projects stay safe.</p></div>
          <GithubRepositoryPicker remoteUrl={remoteUrl} branch={remoteBranch} disabled={busy || remoteWorking} onSelect={(url, branch) => { setRemoteUrl(url); setRemoteBranch(branch); setRemoteFeedback(""); setRestoreName((current) => current || folderName(url.split("/").at(-1)?.replace(/\.git$/, "") ?? "continuum-project")); }} />
          <label>Repository URL<input value={remoteUrl} onChange={(event) => { setRemoteUrl(event.target.value); setRemoteBranch(""); }} placeholder="https://github.com/you/continuum-project.git" required /></label>
          {remoteBranch && <small>Selected branch: {remoteBranch}</small>}
          <label>Restore into<div className="path-picker"><input value={restoreParent} onChange={(event) => setRestoreParent(event.target.value)} /><button type="button" onClick={() => void pick(setRestoreParent, "Choose restore destination")}>Browse…</button></div></label>
          <small>Choose the parent folder where the new project will be saved.</small>
          <label>New local folder name<input value={restoreName} onChange={(event) => setRestoreName(event.target.value)} placeholder="my-restored-project" maxLength={120} /></label>
          <button className="primary-action" disabled={busy || remoteWorking}>{busy || remoteWorking ? "Downloading…" : "Download and verify as new project"}</button>
          {remoteFeedback && <p className={remoteWorking ? "notice" : "remote-warning"} role="status">{remoteFeedback}</p>}
        </form>}
      </section>
    </main>
  );
}
