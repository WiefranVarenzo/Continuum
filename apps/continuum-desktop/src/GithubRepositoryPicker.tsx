import { useEffect, useState } from "react";
import { beginGithubLogin, getGithubLoginProgress, getGithubStatus, listGithubBranches, listGithubRepositories, openGithubAuthorizationPage, openGithubNewRepository, isDesktopRuntime } from "./bridge";
import type { GithubLoginProgress, GithubRepository, GithubStatus } from "./releaseContracts";

interface Props {
  remoteUrl: string;
  branch: string;
  disabled?: boolean;
  onSelect: (remoteUrl: string, branch: string) => void;
}

export function GithubRepositoryPicker({ remoteUrl, branch, disabled, onSelect }: Props) {
  const [status, setStatus] = useState<GithubStatus | null>(null);
  const [progress, setProgress] = useState<GithubLoginProgress | null>(null);
  const [repositories, setRepositories] = useState<GithubRepository[]>([]);
  const [branches, setBranches] = useState<string[]>([]);
  const [working, setWorking] = useState(false);
  const [message, setMessage] = useState("");

  async function refresh() {
    const next = await getGithubStatus();
    setStatus(next);
    setRepositories(next.connected ? await listGithubRepositories() : []);
  }

  useEffect(() => {
    if (!isDesktopRuntime()) return;
    let active = true;
    void getGithubStatus().then(async (next) => {
      if (!active) return;
      setStatus(next);
      if (next.connected) {
        const repos = await listGithubRepositories();
        if (active) setRepositories(repos);
      }
    }).catch((cause) => { if (active) setMessage(String(cause)); });
    return () => { active = false; };
  }, []);

  useEffect(() => {
    if (progress?.phase !== "waiting") return;
    const timer = window.setInterval(() => {
      void getGithubLoginProgress().then(async (next) => {
        setProgress(next);
        if (next.phase === "connected") {
          await refresh();
          setMessage("GitHub connected. Choose a private repository below.");
        } else if (next.phase === "error") {
          setMessage(next.message ?? "GitHub login did not finish.");
        }
      }).catch((cause) => setMessage(String(cause)));
    }, 1200);
    return () => window.clearInterval(timer);
  }, [progress?.phase]);

  useEffect(() => {
    const selected = repositories.find((repo) => repo.clone_url === remoteUrl);
    if (!selected) { setBranches([]); return; }
    let active = true;
    void listGithubBranches(selected.full_name).then((next) => {
      if (active) setBranches(next.length ? next : [selected.default_branch]);
    }).catch(() => { if (active) setBranches([selected.default_branch]); });
    return () => { active = false; };
  }, [repositories, remoteUrl]);

  async function login() {
    setWorking(true);
    setMessage("");
    try {
      const next = await beginGithubLogin();
      setProgress(next);
      if (next.phase === "connected") await refresh();
    }
    catch (cause) { setMessage(String(cause)); }
    finally { setWorking(false); }
  }

  const selected = repositories.find((repo) => repo.clone_url === remoteUrl);
  return <div className="github-picker" aria-label="GitHub connection">
    <div className="github-picker-heading"><strong>GitHub</strong><span>{status?.connected ? `Connected${status.username ? ` as @${status.username}` : ""}` : "Not connected"}</span></div>
    {status && !status.cli_available && <p>GitHub CLI is unavailable. Repair your Continuum installation or install <code>gh</code>, then reopen this page.</p>}
    {status?.cli_available && !status.connected && <>
      <button type="button" disabled={working || progress?.phase === "waiting"} onClick={() => void login()}>Login with GitHub</button>
      <small>GitHub CLI opens your browser and manages the credential. It may request access to repositories beyond this project; Continuum uploads only to the repo you choose. If the OS keyring is unavailable, GitHub CLI may save its credential in a protected local file.</small>
    </>}
    {progress?.phase === "waiting" && <div role="status"><p>Continue in your default browser. {progress.user_code ? <>Enter this one-time code: <strong>{progress.user_code}</strong>.</> : "Preparing the one-time code…"} Continuum will connect automatically after you approve access.</p><button type="button" onClick={() => void openGithubAuthorizationPage().catch((cause) => setMessage(String(cause)))}>Open GitHub authorization page</button>{progress.message && <small>{progress.message}</small>}</div>}
    {status?.connected && <>
      <button type="button" className="github-refresh" disabled={working} onClick={() => { setWorking(true); void refresh().catch((cause) => setMessage(String(cause))).finally(() => setWorking(false)); }}>Refresh repositories</button>
      {repositories.length ? <>
        <label>Choose a private GitHub repository
          <select value={selected?.clone_url ?? ""} disabled={disabled} onChange={(event) => {
            const repo = repositories.find((item) => item.clone_url === event.target.value);
            if (repo) onSelect(repo.clone_url, repo.default_branch);
          }}>
            <option value="">Choose repository…</option>
            {repositories.map((repo) => <option key={repo.full_name} value={repo.clone_url}>{repo.full_name}</option>)}
          </select>
        </label>
        {selected && <label>Branch
          <select value={branch || selected.default_branch} disabled={disabled} onChange={(event) => onSelect(selected.clone_url, event.target.value)}>
            {[...new Set([branch || selected.default_branch, ...branches])].map((name) => <option key={name} value={name}>{name}</option>)}
          </select>
        </label>}
      </> : <div><p>No writable private repository was found. On GitHub, choose Private and leave README, license, and .gitignore empty; then refresh.</p><button type="button" onClick={() => void openGithubNewRepository().catch((cause) => setMessage(String(cause)))}>Open GitHub's new-repository page</button></div>}
    </>}
    {message && <p className="remote-warning" role="status">{message}</p>}
  </div>;
}
