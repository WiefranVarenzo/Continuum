import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { ContinuityPanel } from "./ContinuityPanel";
import type { ContextPack, CurrentProjectState } from "./contextContracts";

const bridge = vi.hoisted(() => ({
  compareCheckpoints: vi.fn(),
  createCheckpoint: vi.fn(),
  getCurrentProjectState: vi.fn(),
  listCheckpoints: vi.fn(),
  previewContextPack: vi.fn(),
  saveContextPack: vi.fn(),
}));

vi.mock("./bridge", () => bridge);

const currentState: CurrentProjectState = {
  project_id: "project-1",
  scope: "research",
  as_of_ledger_sequence: 42,
  computed_at: "2026-09-09T00:00:00Z",
  checkpoint: null,
  freshness: null,
  then: null,
  since: [],
  since_truncated: false,
  now: { counts: { evidence: 2 } },
  next_actions: ["Review the second observation"],
  source_ids: ["source-1", "source-2"],
};

const pack: ContextPack = {
  id: "pack-1",
  schema_version: 1,
  project_id: "project-1",
  scope: "research",
  task: "Resume research",
  audience: "external_ai",
  consumer_target: "external-ai-preview",
  freshness_requirement: "current",
  retrieval_profile: "research",
  checkpoint_id: null,
  source_ledger_sequence: 42,
  generated_at: "2026-09-09T00:00:00Z",
  freshness: null,
  privacy_policy_version: "context-privacy-v1",
  privacy_summary: {},
  budget: { soft_tokens: 16_000, hard_tokens: 32_000, max_bytes: 524_288, max_items: 200, max_item_bytes: 16_384 },
  included_bytes: 360,
  estimated_tokens: 120,
  estimator_id: "utf8_bytes_div_3_ceiling_v1",
  estimator_uncertainty: "medium",
  items: [{
    ordinal: 0,
    tier: 2,
    source_kind: "entity",
    source_id: "source-1",
    source_version: "1",
    classification: "public",
    relevance_basis: ["active_or_unresolved"],
    content: { title: "Public evidence" },
    content_bytes: 360,
    estimated_tokens: 120,
    content_fingerprint: "a".repeat(64),
  }],
  omissions: [{ source_id: "secret-1", reason: "privacy_policy", detail: "Secret source stayed local." }],
  unavailable_sources: [],
  request_fingerprint: "b".repeat(64),
  content_fingerprint: "c".repeat(64),
};

describe("CP10 continuity panel", () => {
  afterEach(cleanup);

  beforeEach(() => {
    vi.clearAllMocks();
    bridge.getCurrentProjectState.mockResolvedValue(currentState);
    bridge.listCheckpoints.mockResolvedValue([]);
    bridge.previewContextPack.mockResolvedValue(pack);
    bridge.saveContextPack.mockResolvedValue({
      pack,
      generated_artifact_id: "artifact-123456789",
      created_by: "desktop-user",
    });
  });

  it("stays idle until a project is opened", () => {
    render(<ContinuityPanel projectOpen={false} capabilities={{ research: false, development: false }} />);
    expect(screen.getByText(/Open a Continuum project/)).toBeInTheDocument();
    expect(bridge.getCurrentProjectState).not.toHaveBeenCalled();
    expect(bridge.previewContextPack).not.toHaveBeenCalled();
  });

  it("loads a valid independent Research scope and presents the resume state", async () => {
    render(<ContinuityPanel projectOpen capabilities={{ research: true, development: false }} />);
    fireEvent.change(screen.getByLabelText("Working area"), { target: { value: "research" } });
    await waitFor(() => expect(bridge.getCurrentProjectState).toHaveBeenCalledWith("research", null));
    expect(screen.getByText("2 evidence")).toBeInTheDocument();
    expect(screen.getByText("Review the second observation")).toBeInTheDocument();
    expect(screen.queryByRole("option", { name: "Connected R&D" })).not.toBeInTheDocument();
  });

  it("previews external context locally, allows removal, and saves only after review", async () => {
    render(<ContinuityPanel projectOpen capabilities={{ research: true, development: true }} />);
    await waitFor(() => expect(bridge.getCurrentProjectState).toHaveBeenCalled());
    fireEvent.change(screen.getByLabelText("Audience"), { target: { value: "external_ai" } });
    fireEvent.click(screen.getByRole("button", { name: "Build local preview" }));
    await screen.findByText("1 included sources");
    expect(bridge.previewContextPack).toHaveBeenCalledWith(expect.objectContaining({ audience: "external_ai" }));
    expect(bridge.saveContextPack).not.toHaveBeenCalled();
    expect(screen.getByText(/Content is not sent by this preview/)).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "Remove" }));
    await waitFor(() => expect(bridge.previewContextPack).toHaveBeenLastCalledWith(expect.objectContaining({ exclude_source_ids: ["source-1"] })));
    fireEvent.click(screen.getByRole("button", { name: "Save verified Context Pack" }));
    await waitFor(() => expect(bridge.saveContextPack).toHaveBeenCalledWith(pack));
  });
});
