import { useState, type FormEvent } from "react";
import { chooseDirectory } from "./bridge";
import type { RepositorySyncSummary, WorkspaceQuestionInput } from "./releaseContracts";

interface Props {
  research: boolean;
  development: boolean;
  busy: boolean;
  onQuestion: (input: WorkspaceQuestionInput) => Promise<void>;
  onRepository: (path: string) => Promise<RepositorySyncSummary>;
  onError: (message: string) => void;
}

export function WorkspaceQuickStart({ research, development, busy, onQuestion, onRepository, onError }: Props) {
  const [title, setTitle] = useState("");
  const [question, setQuestion] = useState("");
  const [context, setContext] = useState("");
  const [desiredOutcome, setDesiredOutcome] = useState("");
  const [repositoryPath, setRepositoryPath] = useState("");
  const [sync, setSync] = useState<RepositorySyncSummary | null>(null);

  async function submitQuestion(event: FormEvent) {
    event.preventDefault();
    try {
      await onQuestion({ title, question, context, desiredOutcome, priority: 2 });
      setTitle("");
      setQuestion("");
      setContext("");
      setDesiredOutcome("");
    } catch (cause) {
      onError(cause instanceof Error ? cause.message : String(cause));
    }
  }

  async function browseRepository() {
    try {
      const selected = await chooseDirectory("Choose a Git repository");
      if (selected) setRepositoryPath(selected);
    } catch (cause) {
      onError(cause instanceof Error ? cause.message : String(cause));
    }
  }

  async function submitRepository(event: FormEvent) {
    event.preventDefault();
    try {
      setSync(await onRepository(repositoryPath));
    } catch (cause) {
      onError(cause instanceof Error ? cause.message : String(cause));
    }
  }

  return <section className="workspace-quick-start" aria-labelledby="workspace-start-title">
    <div className="workspace-start-heading"><div><p className="eyebrow">Start working</p><h2 id="workspace-start-title">Put the first real work into this project</h2></div><p>These actions use the same verified ledger as reports, checkpoints, and AI context.</p></div>
    <div className="workspace-start-grid">
      {research && <form className="workspace-start-card research-start" onSubmit={(event) => void submitQuestion(event)}>
        <div><span>Research</span><h3>Add a research question</h3><p>Define what you are trying to learn before collecting Evidence.</p></div>
        <label>Short title<input value={title} onChange={(event) => setTitle(event.target.value)} maxLength={500} placeholder="Evaluate local-first capture" /></label>
        <label>Question<textarea value={question} onChange={(event) => setQuestion(event.target.value)} maxLength={20000} placeholder="What do we need to find out?" /></label>
        <label>Context<textarea value={context} onChange={(event) => setContext(event.target.value)} maxLength={20000} placeholder="Why this question matters" /></label>
        <label>Desired outcome<input value={desiredOutcome} onChange={(event) => setDesiredOutcome(event.target.value)} maxLength={20000} placeholder="A decision we can make" /></label>
        <button className="primary-action" disabled={busy || !title.trim() || !question.trim()}>{busy ? "Saving…" : "Save question"}</button>
      </form>}
      {development && <form className="workspace-start-card development-start" onSubmit={(event) => void submitRepository(event)}>
        <div><span>Development · local</span><h3>Analyze a Git folder on this computer</h3><p>Continuum reads local commits and code. This does not connect to GitHub/GitLab or upload anything. Project backup and upload are separate in Project settings.</p></div>
        <label>Local Git folder<div className="path-picker"><input value={repositoryPath} onChange={(event) => setRepositoryPath(event.target.value)} placeholder="Choose a local folder containing .git" /><button type="button" onClick={() => void browseRepository()}>Browse…</button></div></label>
        <button className="primary-action" disabled={busy || !repositoryPath.trim()}>{busy ? "Analyzing…" : "Analyze local folder"}</button>
        {sync && <div className="sync-result" aria-live="polite"><strong>Repository synchronized</strong><span>{sync.analyzed_files} files · {sync.code_entities} code entities · {sync.discovered_tests} tests</span><span>{sync.ingested_commits} new commits · {sync.remaining_commits} remaining · analysis {sync.analysis_completeness}</span></div>}
      </form>}
    </div>
  </section>;
}
