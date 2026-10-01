import { useEffect, useMemo, useState, type FormEvent } from "react";
import {
  compareCheckpoints,
  createCheckpoint,
  getCurrentProjectState,
  listCheckpoints,
  previewContextPack,
  saveContextPack,
} from "./bridge";
import type {
  CheckpointComparison,
  CheckpointEnvelope,
  CheckpointScope,
  ContextAudience,
  ContextPack,
  ContextPackRequest,
  CurrentProjectState,
  SpaceCapabilities,
} from "./contextContracts";
import "./continuity.css";

interface Props {
  projectOpen: boolean;
  capabilities: SpaceCapabilities;
}

const labels: Record<CheckpointScope, string> = {
  research: "Research",
  development: "Development",
  integrated: "Connected R&D",
  core: "Whole project",
};

function lines(value: string): string[] {
  return value.split("\n").map((item) => item.trim()).filter(Boolean);
}

function checkpointNote(checkpoint: CheckpointEnvelope): string {
  const note = checkpoint.checkpoint.summary.note;
  return typeof note === "string" && note.trim() ? note : "Saved project boundary";
}

function countSummary(value: Record<string, unknown> | null | undefined): string {
  if (!value) return "No saved state yet";
  const directCounts = value.counts;
  const nestedCounts = value.state && typeof value.state === "object"
    ? (value.state as Record<string, unknown>).counts
    : null;
  const counts = (directCounts ?? nestedCounts) as Record<string, number> | null;
  if (!counts || typeof counts !== "object") return "State is available";
  const entries = Object.entries(counts);
  if (!entries.length) return "No active records";
  return entries.slice(0, 4).map(([kind, count]) => `${count} ${kind.replaceAll("_", " ")}`).join(" · ");
}

function friendlyEvent(eventType: string): string {
  return eventType.replaceAll(".", " ").replaceAll("_", " ");
}

export function ContinuityPanel({ projectOpen, capabilities }: Props) {
  const scopes = useMemo<CheckpointScope[]>(() => {
    const result: CheckpointScope[] = ["core"];
    if (capabilities.research) result.unshift("research");
    if (capabilities.development) result.unshift("development");
    if (capabilities.research && capabilities.development) result.unshift("integrated");
    return result;
  }, [capabilities]);
  const [scope, setScope] = useState<CheckpointScope>("core");
  const [checkpoints, setCheckpoints] = useState<CheckpointEnvelope[]>([]);
  const [state, setState] = useState<CurrentProjectState | null>(null);
  const [selectedCheckpoint, setSelectedCheckpoint] = useState<string>("");
  const [note, setNote] = useState("");
  const [blockers, setBlockers] = useState("");
  const [risks, setRisks] = useState("");
  const [nextActions, setNextActions] = useState("");
  const [task, setTask] = useState("Resume this project and continue the next action");
  const [audience, setAudience] = useState<ContextAudience>("local_user");
  const [includeArtifacts, setIncludeArtifacts] = useState(false);
  const [allowStale, setAllowStale] = useState(false);
  const [pack, setPack] = useState<ContextPack | null>(null);
  const [packRequest, setPackRequest] = useState<ContextPackRequest | null>(null);
  const [compareWith, setCompareWith] = useState("");
  const [comparison, setComparison] = useState<CheckpointComparison | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [notice, setNotice] = useState("");

  useEffect(() => {
    if (!scopes.includes(scope)) setScope(scopes[0]);
  }, [scope, scopes]);

  async function refresh(nextScope = scope, checkpointId: string | null = null) {
    const [nextState, nextCheckpoints] = await Promise.all([
      getCurrentProjectState(nextScope, checkpointId),
      listCheckpoints(nextScope),
    ]);
    setState(nextState);
    setCheckpoints(nextCheckpoints);
    const current = checkpointId ?? nextState.checkpoint?.checkpoint.id ?? "";
    setSelectedCheckpoint(current);
    setCompareWith(nextCheckpoints.find((item) => item.checkpoint.id !== current)?.checkpoint.id ?? "");
  }

  useEffect(() => {
    if (!projectOpen) return;
    let active = true;
    setBusy(true);
    refresh(scope)
      .catch((cause) => active && setError(cause instanceof Error ? cause.message : String(cause)))
      .finally(() => active && setBusy(false));
    return () => { active = false; };
    // refresh is intentionally tied to the selected scope and open project.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [projectOpen, scope]);

  async function chooseCheckpoint(checkpointId: string) {
    setSelectedCheckpoint(checkpointId);
    setPack(null);
    setComparison(null);
    setBusy(true);
    try {
      await refresh(scope, checkpointId || null);
      setError("");
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setBusy(false);
    }
  }

  async function submitCheckpoint(event: FormEvent) {
    event.preventDefault();
    setBusy(true);
    try {
      const created = await createCheckpoint({
        scope,
        trigger: "interruption",
        note: note.trim(),
        blockers: lines(blockers),
        risks: lines(risks),
        next_actions: lines(nextActions),
        semantic_candidate_id: null,
        supersedes_checkpoint_id: selectedCheckpoint || null,
      });
      setNote("");
      setBlockers("");
      setRisks("");
      setNextActions("");
      setPack(null);
      await refresh(scope, created.checkpoint.id);
      setNotice("Smart bookmark saved. You can safely switch work and resume from this boundary later.");
      setError("");
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setBusy(false);
    }
  }

  function makeRequest(excludeSourceIds: string[] = []): ContextPackRequest {
    return {
      task: task.trim(),
      audience,
      consumer_target: audience === "local_user" ? "continuum-desktop" : "external-ai-preview",
      scope,
      checkpoint_id: selectedCheckpoint || null,
      freshness_requirement: allowStale ? "allow_stale_with_warning" : "current",
      retrieval_profile: scope === "core" ? "resume" : scope,
      root_entity_ids: [],
      exclude_source_ids: excludeSourceIds,
      include_artifact_content: includeArtifacts,
      budget: {
        soft_tokens: 16_000,
        hard_tokens: 32_000,
        max_bytes: 512 * 1024,
        max_items: 200,
        max_item_bytes: 16 * 1024,
      },
    };
  }

  async function buildPack(request = makeRequest()) {
    setBusy(true);
    try {
      const preview = await previewContextPack(request);
      setPack(preview);
      setPackRequest(request);
      setNotice("Context Pack preview built locally. Review every included and omitted source before saving.");
      setError("");
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setBusy(false);
    }
  }

  async function removePackItem(sourceId: string) {
    if (!packRequest) return;
    await buildPack({
      ...packRequest,
      exclude_source_ids: [...new Set([...packRequest.exclude_source_ids, sourceId])],
    });
  }

  async function persistPack() {
    if (!pack) return;
    setBusy(true);
    try {
      const saved = await saveContextPack(pack);
      setNotice(`Context Pack saved as a verified project artifact (${saved.generated_artifact_id.slice(0, 8)}…).`);
      setError("");
      await refresh(scope, selectedCheckpoint || null);
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setBusy(false);
    }
  }

  async function runComparison() {
    if (!selectedCheckpoint || !compareWith) return;
    setBusy(true);
    try {
      setComparison(await compareCheckpoints(selectedCheckpoint, compareWith));
      setError("");
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setBusy(false);
    }
  }

  return <section className="continuity-panel" aria-labelledby="continuity-heading">
    <div className="continuity-heading-row">
      <div>
        <p className="eyebrow">CHECKPOINT & CONTEXT</p>
        <h2 id="continuity-heading">Resume without rebuilding your memory</h2>
        <p>Save where you stopped, see what changed, then prepare only the context needed for the next person or AI.</p>
      </div>
      {projectOpen && <label className="scope-picker">Working area<select value={scope} onChange={(event) => { setScope(event.target.value as CheckpointScope); setPack(null); }}>
        {scopes.map((item) => <option value={item} key={item}>{labels[item]}</option>)}
      </select></label>}
    </div>

    {!projectOpen && <p className="continuity-empty">Open a Continuum project to create or resume a smart bookmark.</p>}
    {projectOpen && <>
      {error && <p className="error" role="alert">{error}</p>}
      {notice && <p className="notice" aria-live="polite">{notice}</p>}

      <div className="resume-grid" aria-busy={busy}>
        <article><span>THEN</span><strong>{state?.checkpoint ? checkpointNote(state.checkpoint) : "No bookmark selected"}</strong><small>{countSummary(state?.then)}</small></article>
        <article><span>SINCE</span><strong>{state?.since.length ?? 0} recorded changes</strong><small>{state?.since.slice(-2).map((item) => friendlyEvent(item.event_type)).join(" · ") || "Nothing material changed"}</small></article>
        <article><span>NOW</span><strong>{state?.freshness?.fresh === false ? "Bookmark needs review" : "State is current"}</strong><small>{countSummary(state?.now)}</small></article>
        <article><span>NEXT</span><strong>{state?.next_actions[0] ?? "Choose the next action"}</strong><small>{state && state.next_actions.length > 1 ? `+${state.next_actions.length - 1} more actions` : "Stored with this bookmark"}</small></article>
      </div>

      {state?.freshness && <div className={`freshness ${state.freshness.fresh ? "fresh" : "stale"}`}>
        <strong>{state.freshness.fresh ? "Fresh bookmark" : "Review before resuming"}</strong>
        <span>{state.freshness.fresh ? "Its source boundary still matches the project." : state.freshness.reasons.join(" · ")}</span>
      </div>}

      <div className="continuity-columns">
        <form className="continuity-card" onSubmit={submitCheckpoint}>
          <div className="card-heading"><div><span>1</span><h3>Save a smart bookmark</h3></div><small>Deterministic by default</small></div>
          <label>Where did you stop?<textarea value={note} onChange={(event) => setNote(event.target.value)} placeholder="Example: Evidence is complete; validation remains." /></label>
          <div className="field-pair">
            <label>Blockers, one per line<textarea value={blockers} onChange={(event) => setBlockers(event.target.value)} /></label>
            <label>Risks, one per line<textarea value={risks} onChange={(event) => setRisks(event.target.value)} /></label>
          </div>
          <label>Next actions, one per line<textarea value={nextActions} onChange={(event) => setNextActions(event.target.value)} placeholder="Run validation\nReview the result" /></label>
          <button className="primary-action" disabled={busy}>Save bookmark</button>
        </form>

        <div className="continuity-card">
          <div className="card-heading"><div><span>2</span><h3>Resume or compare</h3></div><small>Then · Since · Now · Next</small></div>
          <label>Resume from<select value={selectedCheckpoint} onChange={(event) => chooseCheckpoint(event.target.value)}>
            <option value="">Latest state</option>
            {checkpoints.map((item) => <option value={item.checkpoint.id} key={item.checkpoint.id}>{checkpointNote(item)} · {new Date(item.checkpoint.created_at).toLocaleString()}</option>)}
          </select></label>
          {checkpoints.length > 1 && <div className="compare-row"><label>Compare with<select value={compareWith} onChange={(event) => setCompareWith(event.target.value)}>
            {checkpoints.filter((item) => item.checkpoint.id !== selectedCheckpoint).map((item) => <option value={item.checkpoint.id} key={item.checkpoint.id}>{checkpointNote(item)}</option>)}
          </select></label><button type="button" disabled={busy || !compareWith} onClick={runComparison}>Compare</button></div>}
          {comparison && <div className="comparison-result" aria-label="checkpoint comparison">
            <strong>What changed between bookmarks</strong>
            <span>{comparison.added_source_ids.length} added · {comparison.changed_source_ids.length} updated · {comparison.removed_source_ids.length} removed</span>
            <small>{comparison.intervening_events.length} auditable events in between</small>
          </div>}
        </div>
      </div>

      <div className="continuity-card pack-builder">
        <div className="card-heading"><div><span>3</span><h3>Prepare a Context Pack</h3></div><small>Preview before saving or sharing</small></div>
        <div className="pack-controls">
          <label>What will happen next?<input value={task} onChange={(event) => setTask(event.target.value)} /></label>
          <label>Audience<select value={audience} onChange={(event) => setAudience(event.target.value as ContextAudience)}>
            <option value="local_user">Only me, on this device</option>
            <option value="external_ai">External AI</option>
            <option value="public_portable">Public / portable</option>
          </select></label>
          <label className="check-control"><input type="checkbox" checked={includeArtifacts} onChange={(event) => setIncludeArtifacts(event.target.checked)} />Include explicitly relevant file content</label>
          <label className="check-control"><input type="checkbox" checked={allowStale} onChange={(event) => setAllowStale(event.target.checked)} />Allow stale bookmark with warning</label>
          <button type="button" className="primary-action" disabled={busy || !task.trim()} onClick={() => buildPack()}>Build local preview</button>
        </div>

        {pack && <div className="pack-preview">
          <div className="pack-meter"><strong>{pack.items.length} included sources</strong><span>~{pack.estimated_tokens.toLocaleString()} / {pack.budget.hard_tokens.toLocaleString()} tokens</span><span>{Math.round(pack.included_bytes / 1024)} KB</span><span>{pack.omissions.length} omitted</span></div>
          <p className="privacy-note">Privacy policy {pack.privacy_policy_version} applied for <strong>{pack.audience.replaceAll("_", " ")}</strong>. Content is not sent by this preview.</p>
          <ol className="pack-items">
            {pack.items.map((item) => <li key={`${item.source_kind}:${item.source_id}`}><div><strong>{item.source_kind.replaceAll("_", " ")}</strong><span>{item.source_id.slice(0, 12)}… · tier {item.tier} · ~{item.estimated_tokens} tokens</span></div><button type="button" onClick={() => removePackItem(item.source_id)} disabled={busy}>Remove</button></li>)}
          </ol>
          {pack.omissions.length > 0 && <details><summary>{pack.omissions.length} omitted sources</summary><ul>{pack.omissions.slice(0, 20).map((item, index) => <li key={`${item.source_id}:${index}`}><strong>{item.reason.replaceAll("_", " ")}</strong> — {item.detail}</li>)}</ul></details>}
          <button type="button" className="primary-action" onClick={persistPack} disabled={busy}>Save verified Context Pack</button>
        </div>}
      </div>
    </>}
  </section>;
}
