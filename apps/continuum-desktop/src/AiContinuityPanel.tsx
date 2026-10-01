import { useEffect, useMemo, useState, type FormEvent } from "react";
import {
  createMcpClientGrant,
  getExternalProposal,
  listExternalProposals,
  listMcpAudit,
  listMcpClientGrants,
  reviewExternalProposal,
  revokeMcpClientGrant,
} from "./bridge";
import type { CheckpointScope, SpaceCapabilities } from "./contextContracts";
import type {
  CreatedMcpGrant,
  ExternalProposal,
  ExternalProposalSummary,
  McpAuditRecord,
  McpClientFamily,
  McpClientGrant,
  McpResourceFamily,
  McpToolName,
  NewMcpClientGrant,
} from "./mcpContracts";
import "./ai-continuity.css";
import { invoke } from "@tauri-apps/api/core";
import { copyText } from "./workspaceBridge";
import { ProposalPreview } from "./ProposalPreview";

interface Props {
  projectOpen: boolean;
  capabilities: SpaceCapabilities;
  preferredClient?: McpClientFamily;
  onUpdated?: () => void;
  projectPath?: string;
  visible?: boolean;
}

const familyLabels: Record<McpClientFamily, string> = {
  codex: "Codex CLI / Codex app",
  claude_code: "Claude Code",
  gemini_cli: "Gemini CLI",
  generic: "Other MCP client",
};

interface InstalledAiConnection {
  family: McpClientFamily;
  client_name: string;
  server_name: string;
  restart_required: boolean;
}

interface AiConnectionInspection {
  family: McpClientFamily;
  client_name: string;
  server_name: string;
  client_detected: boolean;
  configuration_installed: boolean;
  detail: string;
}

function defaultGrant(
  label: string,
  family: McpClientFamily,
  capabilities: SpaceCapabilities,
  allowProposals: boolean,
): NewMcpClientGrant {
  const scopes: CheckpointScope[] = ["core"];
  const resources: McpResourceFamily[] = ["project", "checkpoint", "context", "entity", "provenance", "schema"];
  const tools: McpToolName[] = ["project_get_state", "checkpoint_get", "checkpoint_list", "context_build", "entity_get", "provenance_trace"];
  if (capabilities.research) {
    scopes.unshift("research");
    resources.push("research");
    tools.push("research_search");
  }
  if (capabilities.development) scopes.unshift("development");
  if (capabilities.research && capabilities.development) scopes.unshift("integrated");
  if (allowProposals) tools.push("proposal_submit", "proposal_get_status");
  return {
    client_label: label.trim(),
    client_family: family,
    transport: "stdio",
    allowed_scopes: scopes,
    allowed_resources: resources,
    allowed_tools: tools,
    allow_proposals: allowProposals,
    classification_ceiling: "internal",
    max_artifact_bytes: 0,
    max_request_bytes: 262_144,
    max_response_bytes: 1_048_576,
    max_context_tokens: 16_000,
    max_calls_per_minute: 120,
    tool_timeout_ms: 30_000,
    expires_at: new Date(Date.now() + 7 * 24 * 60 * 60 * 1000).toISOString(),
  };
}

function shortId(value: string): string {
  return value.length > 16 ? `${value.slice(0, 8)}…${value.slice(-5)}` : value;
}

export function AiContinuityPanel({ projectOpen, capabilities, preferredClient, onUpdated, projectPath, visible = true }: Props) {
  const [family, setFamily] = useState<McpClientFamily>("codex");
  const [label, setLabel] = useState(familyLabels.codex);
  const [allowProposals, setAllowProposals] = useState(capabilities.research);
  const [grants, setGrants] = useState<McpClientGrant[]>([]);
  const [proposals, setProposals] = useState<ExternalProposalSummary[]>([]);
  const [selected, setSelected] = useState<ExternalProposal | null>(null);
  const [audit, setAudit] = useState<McpAuditRecord[]>([]);
  const [created, setCreated] = useState<CreatedMcpGrant | null>(null);
  const [reviewNote, setReviewNote] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [notice, setNotice] = useState("");
  const [inspections, setInspections] = useState<Partial<Record<McpClientFamily, AiConnectionInspection>>>({});
  const activeGrants = useMemo(() => grants.filter((grant) => !grant.revoked_at), [grants]);

  useEffect(() => { if (preferredClient) { setFamily(preferredClient); setLabel(familyLabels[preferredClient]); } }, [preferredClient]);
  useEffect(() => { if (capabilities.research) setAllowProposals(true); }, [capabilities.research]);

  useEffect(() => {
    if (!visible || !projectOpen) return;
    const timer = window.setInterval(() => { void refresh(false).catch(cause => setError(String(cause))); }, 8000);
    return () => clearInterval(timer);
  }, [visible, projectOpen]);

  async function copyConnection() {
    if (!created || !projectPath) return;
    try {
      const command = await invoke<string>("workspace_mcp_server");
      await copyText(JSON.stringify({ mcpServers: { continuum: { command, args: ["--project", projectPath], env: { CONTINUUM_MCP_GRANT_TOKEN: created.bearer_token } } } }, null, 2));
      setNotice("MCP server settings copied, including the access token. Add them to your client's private MCP configuration. Keep the token out of chats and repositories.");
    } catch (cause) { setError(String(cause)); }
  }

  async function refresh(checkClientSetup = false) {
    const [nextGrants, nextProposals, nextAudit] = await Promise.all([
      listMcpClientGrants(),
      listExternalProposals("pending"),
      listMcpAudit(),
    ]);
    setGrants(nextGrants);
    setProposals(nextProposals);
    setAudit(nextAudit.slice(0, 10));
    if (checkClientSetup) {
      const families = [...new Set(nextGrants.filter((grant) => !grant.revoked_at && grant.client_family !== "generic").map((grant) => grant.client_family))];
      const checked = await Promise.all(families.map(async (clientFamily) => {
        const grant = nextGrants.find((item) => !item.revoked_at && item.client_family === clientFamily)!;
        const inspection = await invoke<AiConnectionInspection>("inspect_ai_client_connection", { family: clientFamily, projectId: grant.project_id });
        return [clientFamily, inspection] as const;
      }));
      setInspections(Object.fromEntries(checked));
    }
  }

  async function checkConnection(grant: McpClientGrant) {
    setBusy(true);
    try {
      if (grant.client_family === "generic") {
        setNotice(grant.last_used_at ? "This generic MCP grant has recorded real client activity." : "No client call has used this manual grant yet.");
        return;
      }
      const inspection = await invoke<AiConnectionInspection>("inspect_ai_client_connection", { family: grant.client_family, projectId: grant.project_id });
      setInspections((current) => ({ ...current, [grant.client_family]: inspection }));
      setNotice(inspection.configuration_installed
        ? `${inspection.detail} ${grant.last_used_at ? `A real MCP call was recorded at ${new Date(grant.last_used_at).toLocaleString()}.` : "No real MCP tool call has been recorded yet; run Organize with AI to prove the full path."}`
        : `${inspection.detail} Disconnect this stale access and reconnect it to repair the private client configuration.`);
      setError("");
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setBusy(false);
    }
  }

  useEffect(() => {
    if (!projectOpen || !visible) return;
    let active = true;
    setBusy(true);
    refresh(true)
      .catch((cause) => active && setError(cause instanceof Error ? cause.message : String(cause)))
      .finally(() => active && setBusy(false));
    return () => { active = false; };
  }, [projectOpen, visible]);

  async function createGrant(event: FormEvent) {
    event.preventDefault();
    if (!label.trim()) return;
    setBusy(true);
    try {
      const automatic = family !== "generic";
      const sameFamily = activeGrants.filter((grant) => grant.client_family === family);
      const currentInspection = inspections[family];
      const healthy = sameFamily.length === 1
        && Boolean(sameFamily[0].last_used_at)
        && Boolean(currentInspection?.configuration_installed)
        && (!allowProposals || sameFamily[0].allow_proposals);
      if (automatic && healthy) {
        setNotice(`${familyLabels[family]} is already connected, verified, and has the requested permissions. No duplicate access was created.`);
        return;
      }
      if (automatic && sameFamily.length > 0) {
        const reason = sameFamily.length > 1 ? `${sameFamily.length} overlapping accesses` : "an incomplete or under-permissioned access";
        if (!window.confirm(`Repair ${familyLabels[family]}? Continuum found ${reason}. It will revoke those project accesses, replace the saved MCP entry, and create one clean connection.`)) return;
        for (const grant of sameFamily) await revokeMcpClientGrant(grant.id);
        await invoke("remove_ai_client_connection", { family, projectId: sameFamily[0].project_id }).catch(() => undefined);
      }
      const clientLabel = automatic ? `${familyLabels[family]} · automatic` : label;
      const result = await createMcpClientGrant(defaultGrant(clientLabel, family, capabilities, allowProposals));
      if (automatic) {
        try {
          const installed = await invoke<InstalledAiConnection>("install_ai_client_connection", {
            family,
            bearerToken: result.bearer_token,
          });
          setCreated(null);
          setNotice(`${installed.client_name} is connected as “${installed.server_name}”. Restart the AI client once, then Continuum tools will appear automatically. The access secret was installed privately and was not shown or copied.`);
        } catch (cause) {
          await revokeMcpClientGrant(result.grant.id).catch(() => undefined);
          throw cause;
        }
      } else {
        setCreated(result);
        setNotice("Manual access created. Copy the one-time settings now; Continuum will not show the secret again.");
      }
      onUpdated?.();
      setError("");
      await refresh(true);
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setBusy(false);
    }
  }

  async function revoke(grant: McpClientGrant) {
    setBusy(true);
    try {
      await revokeMcpClientGrant(grant.id);
      let cleanupWarning = "";
      if (grant.client_label.endsWith("· automatic") && grant.client_family !== "generic") {
        try {
          await invoke("remove_ai_client_connection", { family: grant.client_family, projectId: grant.project_id });
        } catch (cause) {
          cleanupWarning = ` The saved client entry could not be removed automatically, but its access is already invalid: ${String(cause)}`;
        }
      }
      setCreated((current) => current?.grant.id === grant.id ? null : current);
      setNotice(`Access for ${grant.client_label} was revoked.${cleanupWarning}`);
      setError("");
      await refresh(true);
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setBusy(false);
    }
  }

  async function inspectProposal(proposalId: string) {
    setBusy(true);
    try {
      setSelected(await getExternalProposal(proposalId));
      setReviewNote("");
      setError("");
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setBusy(false);
    }
  }

  async function review(decision: "accept" | "reject") {
    if (!selected) return;
    setBusy(true);
    try {
      const reviewed = await reviewExternalProposal(selected.id, selected.version, decision, reviewNote);
      let materialized = "";
      if (decision === "accept" && ["research_note", "finding_candidate", "relationship_candidate", "research_synthesis", "diagram_plan"].includes(reviewed.kind)) {
        try {
          materialized = await invoke<string>("materialize_board_proposal", { proposalId: reviewed.id });
        } catch (cause) {
          setSelected(reviewed);
          setError(`The proposal was accepted, but the board update needs attention: ${String(cause)}`);
          await refresh();
          return;
        }
      }
      onUpdated?.();
      setSelected(null);
      setReviewNote("");
      setNotice(decision === "accept"
        ? materialized || "Proposal accepted as reviewed project guidance."
        : "Proposal rejected; canonical project data was not changed.");
      setError("");
      await refresh();
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setBusy(false);
    }
  }

  async function applyAccepted() {
    if (!selected || selected.status !== "accepted") return;
    setBusy(true);
    try {
      const applied = await invoke<string>("materialize_board_proposal", { proposalId: selected.id });
      setNotice(applied); setSelected(null); setError(""); onUpdated?.(); await refresh();
    } catch (cause) {
      setError(`The accepted proposal could not update the board: ${String(cause)}`);
    } finally { setBusy(false); }
  }

  return <section className="ai-continuity-panel" aria-labelledby="ai-continuity-title">
    <div className="ai-heading-row">
      <div><p className="eyebrow">CP11 · Permissioned AI continuity</p><h2 id="ai-continuity-title">Connect an AI safely</h2><p>Give a local MCP client bounded, revocable access. AI reads verified context and submits proposals; it never edits canonical project knowledge directly.</p></div>
      <span className="ai-status">{activeGrants.length} active access · {proposals.length} pending</span>
    </div>
    {!projectOpen ? <p className="ai-empty">Open a Continuum project before managing AI access.</p> : <>
      {error && <p className="error" role="alert">{error}</p>}
      {notice && <p className="notice" aria-live="polite">{notice}</p>}
      {created && <div className="token-reveal" role="status">
        <strong>One-time token for {created.grant.client_label}</strong>
        <code>{created.bearer_token}</code>
        <div className="ai-actions"><button type="button" onClick={() => void copyText(created.bearer_token).then(() => setNotice("Token copied.")).catch(cause => setError(String(cause)))}>Copy token</button>{projectPath && <button type="button" onClick={() => void copyConnection()}>Copy MCP server settings</button>}<button type="button" onClick={() => setCreated(null)}>I saved it</button></div>
        <small>Put it in the client environment as <code>CONTINUUM_MCP_GRANT_TOKEN</code>. Never place it in source control, prompts, logs, or command-line arguments.</small>
      </div>}
      <div className="ai-grid">
        <form className="ai-card" onSubmit={createGrant}>
          <h3>Connect an AI</h3>
          <label>AI client<select aria-label="AI client" value={family} onChange={(event) => { const next = event.target.value as McpClientFamily; setFamily(next); setLabel(familyLabels[next]); setCreated(null); }}>{Object.entries(familyLabels).map(([value, text]) => <option key={value} value={value}>{text}</option>)}</select></label>
          {family === "generic" && <label>Access label<input aria-label="Access label" maxLength={200} value={label} onChange={(event) => setLabel(event.target.value)} /></label>}
          <label className="ai-check"><input type="checkbox" checked={allowProposals} onChange={(event) => setAllowProposals(event.target.checked)} /> Allow reviewed AI suggestions</label>
          {capabilities.research && !allowProposals && <p className="ai-warning">Read-only access cannot describe Evidence, create Findings, recommend next steps, or generate the reviewed research diagram.</p>}
          <p className="ai-policy">Local STDIO · expires in 7 days · Internal ceiling · 16K-token context · no artifact content</p>
          <button className="primary-action" disabled={busy || !label.trim()}>{family === "generic" ? "Create manual access" : activeGrants.some(grant => grant.client_family === family) ? `Repair ${familyLabels[family]} connection` : `Connect ${familyLabels[family]}`}</button>
          {family === "codex" && <small>This installs Continuum in the local Codex MCP configuration. Restart Codex or start a new Codex task after connecting. ChatGPT web or a separate ChatGPT session requires a remote MCP/plugin and does not inherit this local STDIO entry.</small>}
          {family !== "generic" && <small>Continuum creates a scoped secret and installs the MCP settings behind the scenes. You never need to copy the token.</small>}
        </form>
        <div className="ai-card">
          <h3>Connection proof</h3>
          <p className="ai-explainer">Three states are checked separately: the client exists, its project MCP entry is installed, and at least one real tool call reached Continuum.</p>
          {activeGrants.some((grant, index, all) => all.findIndex(other => other.client_family === grant.client_family) !== index) && <p className="ai-warning">Overlapping access records were found. Use “Repair connection” once to replace them with one clean, proposal-capable connection.</p>}
          {!activeGrants.length ? <p className="ai-empty">No AI client can access this project.</p> : <ul className="access-list">{activeGrants.map((grant) => {
            const inspection = inspections[grant.client_family];
            const proof = grant.client_family !== "generic" && inspection && !inspection.configuration_installed
              ? { tone: "missing", text: "Saved connection uses a missing or outdated server · repair required" }
              : grant.last_used_at
                ? { tone: "verified", text: `Verified tool activity · ${new Date(grant.last_used_at).toLocaleString()}` }
                : inspection?.configuration_installed
                ? { tone: "waiting", text: "Current server installed · restart Codex, then make one tool call" }
                : grant.client_family === "generic"
                  ? { tone: "waiting", text: "Manual access created · no MCP tool call recorded" }
                  : { tone: "missing", text: inspection ? "Project MCP configuration is missing" : "Checking private client configuration…" };
            return <li key={grant.id}>
              <div><strong>{grant.client_label}</strong><span>{familyLabels[grant.client_family]} · {grant.allow_proposals ? "read + propose" : "read only"}</span><em className={`connection-proof ${proof.tone}`}>{proof.text}</em><small>{shortId(grant.id)} · expires {new Date(grant.expires_at).toLocaleString()}</small></div>
              <div className="access-actions"><button type="button" disabled={busy} onClick={() => void checkConnection(grant)}>Check setup</button><button type="button" disabled={busy} onClick={() => revoke(grant)}>{grant.client_label.endsWith("· automatic") ? "Disconnect" : "Revoke"}</button></div>
            </li>;
          })}</ul>}
        </div>
      </div>
      <div className="ai-grid lower-grid">
        <div className="ai-card">
          <h3>Proposal review</h3>
          {!proposals.length ? <p className="ai-empty">No proposal is waiting for human review.</p> : <ul className="proposal-list">{proposals.map((proposal) => <li key={proposal.id}><button type="button" onClick={() => inspectProposal(proposal.id)}><strong>{proposal.title}</strong><span>{proposal.kind.replaceAll("_", " ")} · {proposal.scope}</span></button></li>)}</ul>}
          {selected && <div className="proposal-review"><h4>{selected.title}</h4><p>{selected.rationale}</p><ProposalPreview proposal={selected} /><details><summary>Technical details · {selected.source_refs.length} sources</summary><pre>{JSON.stringify(selected.payload, null, 2)}</pre><p>{selected.source_refs.join(", ")}</p></details>{selected.status === "pending" ? <><label>Review note<textarea aria-label="Review note" maxLength={20_000} value={reviewNote} onChange={(event) => setReviewNote(event.target.value)} /></label><div className="ai-actions"><button type="button" disabled={busy} onClick={() => review("reject")}>Reject</button><button type="button" className="primary-action" disabled={busy} onClick={() => review("accept")}>Accept reviewed proposal</button></div></> : <div className="ai-actions"><button type="button" onClick={() => setSelected(null)}>Close</button><button type="button" className="primary-action" disabled={busy} onClick={() => void applyAccepted()}>Apply accepted proposal to board</button></div>}</div>}
        </div>
        <div className="ai-card">
          <h3>Recent safe audit</h3>
          {!audit.length ? <p className="ai-empty">No MCP activity recorded yet.</p> : <ol className="audit-list">{audit.map((record) => <li key={record.id}><strong>{record.method}</strong><span className={`audit-${record.outcome}`}>{record.outcome}</span><small>{record.event_type} · {record.duration_ms} ms · {new Date(record.occurred_at).toLocaleString()}</small></li>)}</ol>}
        </div>
      </div>
    </>}
  </section>;
}
