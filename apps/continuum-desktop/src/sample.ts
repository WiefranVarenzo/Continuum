import type { HumanDocument } from "./contracts";

export const sampleDocument: HumanDocument = {
  id: "demo-report",
  schema_version: 1,
  project_id: "continuum-demo",
  kind: "integrated_report",
  title: "Continuum — Connected R&D Overview",
  audience: "local_project",
  source_checkpoint_id: "CP-008",
  source_ledger_sequence: 128,
  generated_at: "2026-09-08T09:00:00Z",
  classification: "internal",
  omissions: [],
  limitations: ["Demo data is illustrative; the Rust core supplies verified project state in production."],
  citations: [
    { source_kind: "entity", source_id: "RQ-01", source_type: "research_question", title: "How can research resume without context loss?", status: "active", origin: "user", version: "1", classification: "internal" },
    { source_kind: "entity", source_id: "D-01", source_type: "decision", title: "Use durable semantic checkpoints", status: "accepted", origin: "user", version: "2", classification: "internal" },
    { source_kind: "entity", source_id: "CS-01", source_type: "change_set", title: "Implement checkpoint persistence", status: "observed", origin: "git_observed", version: "1", classification: "internal" },
  ],
  blocks: [
    { meta: { id: "overview", title: "Where we are", source_ids: ["RQ-01", "D-01", "CS-01"], contribution: { kind: "deterministic", actor_id: "continuum-core" } }, content: { type: "overview", summary: "Research decisions are connected to the implementation that carries them forward. Resume from the checkpoint without rereading the whole history.", status: "active", blockers: [], next_actions: ["Capture the next validation result", "Continue CP9 research capture"] } },
    { meta: { id: "status", title: "Current state", source_ids: ["RQ-01", "D-01", "CS-01"], contribution: { kind: "deterministic" } }, content: { type: "status_cards", cards: [{ label: "Research questions", value: "1", state: "neutral", source_ids: ["RQ-01"] }, { label: "Accepted decisions", value: "1", state: "success", source_ids: ["D-01"] }, { label: "Observed changes", value: "1", state: "success", source_ids: ["CS-01"] }] } },
    { meta: { id: "knowledge-graph", title: "Connected knowledge", source_ids: ["RQ-01", "D-01", "CS-01"], contribution: { kind: "deterministic" } }, content: { type: "graph", specification: { renderer: "react_flow_elk", layout: "org.eclipse.elk.layered", direction: "left_right", truncated: false, textual_alternative: "The research question leads to an accepted decision, which is implemented by the ChangeSet.", nodes: [{ id: "RQ-01", label: "Resume without context loss", kind: "research_question", status: "active", source_ids: ["RQ-01"] }, { id: "D-01", label: "Use semantic checkpoints", kind: "decision", status: "accepted", source_ids: ["D-01"] }, { id: "CS-01", label: "Checkpoint persistence", kind: "change_set", status: "observed", source_ids: ["CS-01"] }], edges: [{ id: "e1", source: "RQ-01", target: "D-01", label: "answered by", source_ids: ["RQ-01", "D-01"] }, { id: "e2", source: "D-01", target: "CS-01", label: "implemented by", source_ids: ["D-01", "CS-01"] }] } } },
    { meta: { id: "portable-flow", title: "Connected R&D continuity loop", source_ids: ["RQ-01", "D-01", "CS-01"], contribution: { kind: "deterministic" } }, content: { type: "mermaid_diagram", specification: { diagram_kind: "flowchart", direction: "left_right", textual_alternative: "Research evidence becomes an accepted decision, the decision becomes a development change, and validated learning returns to the next research cycle.", nodes: [{ id: "research", label: "Research\nWhy & evidence", kind: "research", status: "active", source_ids: ["RQ-01"] }, { id: "decision", label: "Accepted decision", kind: "decision", status: "accepted", source_ids: ["D-01"] }, { id: "development", label: "Development\nWhat & validation", kind: "development", status: "observed", source_ids: ["CS-01"] }, { id: "checkpoint", label: "Continuity checkpoint", kind: "checkpoint", status: "verified", source_ids: ["RQ-01", "D-01", "CS-01"] }, { id: "feedback", label: "Learning feedback", kind: "feedback", status: "active", source_ids: ["RQ-01", "CS-01"] }], edges: [{ id: "f1", source: "research", target: "decision", label: "supports", source_ids: ["RQ-01", "D-01"] }, { id: "f2", source: "decision", target: "development", label: "guides", source_ids: ["D-01", "CS-01"] }, { id: "f3", source: "development", target: "checkpoint", label: "validated into", source_ids: ["CS-01"] }, { id: "f4", source: "checkpoint", target: "feedback", label: "preserves", source_ids: ["RQ-01", "CS-01"] }, { id: "f5", source: "feedback", target: "research", label: "resumes", source_ids: ["RQ-01"] }] } } },
    { meta: { id: "timeline", title: "Timeline", source_ids: ["RQ-01", "D-01", "CS-01"], contribution: { kind: "deterministic" } }, content: { type: "timeline", truncated: false, entries: [{ id: "t1", occurred_at: "2026-09-06T09:00:00Z", label: "Question captured", detail: "Research space", state: "active", source_ids: ["RQ-01"] }, { id: "t2", occurred_at: "2026-09-07T09:00:00Z", label: "Decision accepted", detail: "Continuity Core", state: "accepted", source_ids: ["D-01"] }, { id: "t3", occurred_at: "2026-09-08T09:00:00Z", label: "Implementation observed", detail: "Development space", state: "observed", source_ids: ["CS-01"] }] } },
    { meta: { id: "citations", title: "Sources and verification", source_ids: ["RQ-01", "D-01", "CS-01"], contribution: { kind: "deterministic" } }, content: { type: "citation_list", citation_ids: ["RQ-01", "D-01", "CS-01"] } },
  ],
  material_fingerprint: "demo-only",
};
