import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { AiContinuityPanel } from "./AiContinuityPanel";
import type { ExternalProposal } from "./mcpContracts";

const bridge = vi.hoisted(() => ({
  createMcpClientGrant: vi.fn(),
  getExternalProposal: vi.fn(),
  listExternalProposals: vi.fn(),
  listMcpAudit: vi.fn(),
  listMcpClientGrants: vi.fn(),
  reviewExternalProposal: vi.fn(),
  revokeMcpClientGrant: vi.fn(),
}));
const native = vi.hoisted(() => ({ invoke: vi.fn() }));

vi.mock("./bridge", () => bridge);
vi.mock("@tauri-apps/api/core", () => native);
vi.mock("./workspaceBridge", () => ({ copyText: vi.fn().mockResolvedValue(undefined) }));

const grant = {
  id: "grant-1234567890",
  project_id: "project-1",
  client_label: "Codex local",
  client_family: "codex" as const,
  transport: "stdio" as const,
  allowed_scopes: ["research", "core"] as const,
  allowed_resources: ["project", "checkpoint", "context", "schema"] as const,
  allowed_tools: ["project_get_state", "checkpoint_list", "context_build"] as const,
  allow_proposals: false,
  classification_ceiling: "internal" as const,
  max_artifact_bytes: 0,
  max_request_bytes: 262_144,
  max_response_bytes: 1_048_576,
  max_context_tokens: 16_000,
  max_calls_per_minute: 120,
  tool_timeout_ms: 30_000,
  created_at: "2026-09-10T00:00:00Z",
  expires_at: "2026-09-17T00:00:00Z",
  last_used_at: null,
  revoked_at: null,
  created_by: "desktop-user",
  version: 1,
};

const proposal: ExternalProposal = {
  id: "proposal-1",
  project_id: "project-1",
  grant_id: grant.id,
  session_id: "session-1",
  idempotency_key: "once",
  kind: "next_action",
  scope: "research",
  title: "Review unexplained measurement",
  rationale: "The current evidence contains one unresolved result.",
  payload: { next_action: "Repeat the measurement" },
  source_refs: ["evidence-1"],
  payload_fingerprint: "a".repeat(64),
  status: "pending",
  review_note: null,
  reviewed_by: null,
  created_at: "2026-09-10T00:00:00Z",
  reviewed_at: null,
  expires_at: "2026-09-17T00:00:00Z",
  version: 1,
};

describe("CP11 AI continuity panel", () => {
  afterEach(cleanup);

  beforeEach(() => {
    vi.clearAllMocks();
    bridge.listMcpClientGrants.mockResolvedValue([]);
    bridge.listExternalProposals.mockResolvedValue([]);
    bridge.listMcpAudit.mockResolvedValue([]);
    bridge.createMcpClientGrant.mockResolvedValue({ grant, bearer_token: "ctmcp_one_time_secret" });
    bridge.revokeMcpClientGrant.mockResolvedValue({ ...grant, revoked_at: "2026-09-10T01:00:00Z" });
    bridge.getExternalProposal.mockResolvedValue(proposal);
    bridge.reviewExternalProposal.mockResolvedValue({ ...proposal, status: "accepted", version: 2 });
    native.invoke.mockResolvedValue({
      family: "codex",
      client_name: "Codex CLI / Codex app",
      server_name: "continuum-project-1",
      restart_required: true,
    });
  });

  it("stays closed until a project is opened", () => {
    render(<AiContinuityPanel projectOpen={false} capabilities={{ research: false, development: false }} />);
    expect(screen.getByText(/Open a Continuum project/)).toBeInTheDocument();
    expect(bridge.listMcpClientGrants).not.toHaveBeenCalled();
  });

  it("installs a bounded proposal-capable connection without exposing its secret", async () => {
    render(<AiContinuityPanel projectOpen capabilities={{ research: true, development: false }} projectPath="/projects/one" />);
    await waitFor(() => expect(bridge.listMcpClientGrants).toHaveBeenCalled());
    fireEvent.click(screen.getByRole("button", { name: "Connect Codex CLI / Codex app" }));

    await screen.findByText(/is connected as/);
    expect(bridge.createMcpClientGrant).toHaveBeenCalledWith(expect.objectContaining({
      client_family: "codex",
      transport: "stdio",
      allowed_scopes: ["research", "core"],
      allow_proposals: true,
      classification_ceiling: "internal",
      max_artifact_bytes: 0,
      max_context_tokens: 16_000,
    }));
    expect(native.invoke).toHaveBeenCalledWith("install_ai_client_connection", {
      family: "codex",
      bearerToken: "ctmcp_one_time_secret",
    });
    expect(screen.queryByText("ctmcp_one_time_secret")).not.toBeInTheDocument();
  });

  it("keeps one-time token setup only as an explicit manual fallback", async () => {
    render(<AiContinuityPanel projectOpen capabilities={{ research: true, development: false }} />);
    await waitFor(() => expect(bridge.listMcpClientGrants).toHaveBeenCalled());
    fireEvent.change(screen.getByLabelText("AI client"), { target: { value: "generic" } });
    fireEvent.change(screen.getByLabelText("Access label"), { target: { value: "Local MCP app" } });
    fireEvent.click(screen.getByRole("button", { name: "Create manual access" }));
    await screen.findByText("ctmcp_one_time_secret");
    expect(native.invoke).not.toHaveBeenCalledWith("install_ai_client_connection", expect.anything());
  });

  it("revokes the fresh grant if automatic client setup fails", async () => {
    native.invoke.mockRejectedValueOnce(new Error("not installed"));
    render(<AiContinuityPanel projectOpen capabilities={{ research: true, development: false }} />);
    await waitFor(() => expect(bridge.listMcpClientGrants).toHaveBeenCalled());
    fireEvent.click(screen.getByRole("button", { name: "Connect Codex CLI / Codex app" }));
    await screen.findByRole("alert");
    expect(bridge.revokeMcpClientGrant).toHaveBeenCalledWith(grant.id);
    expect(screen.queryByText("ctmcp_one_time_secret")).not.toBeInTheDocument();
  });

  it("does not let old tool activity hide an outdated MCP server", async () => {
    bridge.listMcpClientGrants.mockResolvedValue([{ ...grant, last_used_at: "2026-09-24T01:00:00Z" }]);
    native.invoke.mockResolvedValue({
      family: "codex",
      client_name: "Codex CLI / Codex app",
      server_name: "continuum-project-1",
      client_detected: true,
      configuration_installed: false,
      detail: "Codex still points to an older Continuum MCP binary.",
    });
    render(<AiContinuityPanel projectOpen capabilities={{ research: true, development: false }} />);
    expect(await screen.findByText(/outdated server · repair required/)).toBeVisible();
    expect(screen.queryByText(/Verified tool activity/)).not.toBeInTheDocument();
  });

  it("requires an explicit human review before accepting a proposal", async () => {
    bridge.listExternalProposals.mockResolvedValue([proposal]);
    render(<AiContinuityPanel projectOpen capabilities={{ research: true, development: true }} />);
    await screen.findByText(proposal.title);
    fireEvent.click(screen.getByRole("button", { name: new RegExp(proposal.title) }));
    await screen.findByText(proposal.rationale);
    fireEvent.change(screen.getByLabelText("Review note"), { target: { value: "Evidence checked" } });
    fireEvent.click(screen.getByRole("button", { name: "Accept reviewed proposal" }));
    await waitFor(() => expect(bridge.reviewExternalProposal).toHaveBeenCalledWith(
      proposal.id,
      proposal.version,
      "accept",
      "Evidence checked",
    ));
  });

  it("materializes an accepted AI description only after human approval", async () => {
    const noteProposal: ExternalProposal = {
      ...proposal,
      kind: "research_note",
      title: "Describe screenshot",
      payload: { entity_id: "evidence-1", suggested_description: "A visible workflow diagram." },
    };
    bridge.listExternalProposals.mockResolvedValue([noteProposal]);
    bridge.getExternalProposal.mockResolvedValue(noteProposal);
    bridge.reviewExternalProposal.mockResolvedValue({ ...noteProposal, status: "accepted", version: 2 });
    native.invoke.mockResolvedValueOnce("Description applied to Saved screenshot");
    render(<AiContinuityPanel projectOpen capabilities={{ research: true, development: false }} />);
    fireEvent.click(await screen.findByRole("button", { name: new RegExp(noteProposal.title) }));
    await screen.findByText(noteProposal.rationale);
    fireEvent.click(screen.getByRole("button", { name: "Accept reviewed proposal" }));
    await waitFor(() => expect(native.invoke).toHaveBeenCalledWith("materialize_board_proposal", { proposalId: noteProposal.id }));
    expect(await screen.findByText("Description applied to Saved screenshot")).toBeVisible();
  });
});
