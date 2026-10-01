import type { CheckpointScope } from "./contextContracts";

export type McpClientFamily = "codex" | "claude_code" | "gemini_cli" | "generic";
export type McpTransport = "stdio" | "streamable_http";
export type McpClassificationCeiling = "public" | "internal" | "sensitive";
export type McpResourceFamily = "project" | "checkpoint" | "context" | "entity" | "research" | "provenance" | "documentation" | "schema";
export type McpToolName =
  | "project_get_state"
  | "checkpoint_get"
  | "checkpoint_list"
  | "context_build"
  | "entity_get"
  | "research_search"
  | "provenance_trace"
  | "proposal_submit"
  | "proposal_get_status";

export interface NewMcpClientGrant {
  client_label: string;
  client_family: McpClientFamily;
  transport: McpTransport;
  allowed_scopes: CheckpointScope[];
  allowed_resources: McpResourceFamily[];
  allowed_tools: McpToolName[];
  allow_proposals: boolean;
  classification_ceiling: McpClassificationCeiling;
  max_artifact_bytes: number;
  max_request_bytes: number;
  max_response_bytes: number;
  max_context_tokens: number;
  max_calls_per_minute: number;
  tool_timeout_ms: number;
  expires_at: string;
}

export interface McpClientGrant extends NewMcpClientGrant {
  id: string;
  project_id: string;
  created_at: string;
  last_used_at: string | null;
  revoked_at: string | null;
  created_by: string;
  version: number;
}

export interface CreatedMcpGrant {
  grant: McpClientGrant;
  bearer_token: string;
}

export interface ExternalProposalSummary {
  id: string;
  grant_id: string;
  kind: "research_note" | "finding_candidate" | "decision_candidate" | "requirement_candidate" | "relationship_candidate" | "research_synthesis" | "diagram_plan" | "next_action";
  scope: CheckpointScope;
  title: string;
  status: "pending" | "accepted" | "rejected" | "expired";
  created_at: string;
  expires_at: string;
  version: number;
}

export interface ExternalProposal extends ExternalProposalSummary {
  project_id: string;
  session_id: string | null;
  idempotency_key: string;
  rationale: string;
  payload: unknown;
  source_refs: string[];
  payload_fingerprint: string;
  review_note: string | null;
  reviewed_by: string | null;
  reviewed_at: string | null;
}

export interface McpAuditRecord {
  id: string;
  grant_id: string | null;
  session_id: string | null;
  correlation_id: string;
  event_type: string;
  method: string;
  target: string;
  outcome: string;
  request_bytes: number;
  response_bytes: number;
  duration_ms: number;
  safe_detail: unknown;
  occurred_at: string;
}
