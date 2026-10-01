export type CheckpointScope = "research" | "development" | "integrated" | "core";
export type CheckpointTrigger = "manual" | "interruption" | "handoff" | "milestone" | "policy";
export type ContextAudience = "local_user" | "external_ai" | "public_portable";
export type FreshnessRequirement = "current" | "allow_stale_with_warning";
export type RetrievalProfile = "resume" | "research" | "development" | "integrated" | "custom";

export interface SpaceCapabilities {
  research: boolean;
  development: boolean;
}

export interface CheckpointRecord {
  id: string;
  project_id: string;
  scope: string;
  ledger_sequence: number;
  summary: Record<string, unknown>;
  created_at: string;
  created_by: string;
}

export interface CheckpointEnvelope {
  checkpoint: CheckpointRecord;
  schema_version: number;
  trigger: string;
  deterministic: Record<string, unknown>;
  semantic_candidate_id: string | null;
  semantic_snapshot: unknown | null;
  privacy_policy_version: string;
  privacy_snapshot: Record<string, unknown>;
  repository_snapshot: Record<string, unknown>;
  capture_snapshot: Record<string, unknown>;
  material_fingerprint: string;
  supersedes_checkpoint_id: string | null;
}

export interface CheckpointFreshness {
  checkpoint_id: string;
  fresh: boolean;
  evaluated_at_ledger_sequence: number;
  events_since_checkpoint: number;
  relevant_events_truncated: boolean;
  changed_source_ids: string[];
  missing_source_ids: string[];
  changed_artifact_ids: string[];
  repository_diverged_ids: string[];
  repository_unavailable_ids: string[];
  reasons: string[];
}

export interface CheckpointEventDelta {
  ledger_sequence: number;
  event_type: string;
  aggregate_id: string | null;
  occurred_at: string;
}

export interface CurrentProjectState {
  project_id: string;
  scope: string;
  as_of_ledger_sequence: number;
  computed_at: string;
  checkpoint: CheckpointEnvelope | null;
  freshness: CheckpointFreshness | null;
  then: Record<string, unknown> | null;
  since: CheckpointEventDelta[];
  since_truncated: boolean;
  now: Record<string, unknown>;
  next_actions: string[];
  source_ids: string[];
}

export interface ContextBudget {
  soft_tokens: number;
  hard_tokens: number;
  max_bytes: number;
  max_items: number;
  max_item_bytes: number;
}

export interface ContextPackRequest {
  task: string;
  audience: ContextAudience;
  consumer_target: string;
  scope: CheckpointScope;
  checkpoint_id: string | null;
  freshness_requirement: FreshnessRequirement;
  retrieval_profile: RetrievalProfile;
  root_entity_ids: string[];
  exclude_source_ids: string[];
  include_artifact_content: boolean;
  budget: ContextBudget;
}

export interface ContextPackItem {
  ordinal: number;
  tier: number;
  source_kind: string;
  source_id: string;
  source_version: string;
  classification: string;
  relevance_basis: string[];
  content: Record<string, unknown>;
  content_bytes: number;
  estimated_tokens: number;
  content_fingerprint: string;
}

export interface ContextOmission {
  source_id: string | null;
  reason: string;
  detail: string;
}

export interface ContextPack {
  id: string;
  schema_version: number;
  project_id: string;
  scope: string;
  task: string;
  audience: string;
  consumer_target: string;
  freshness_requirement: string;
  retrieval_profile: string;
  checkpoint_id: string | null;
  source_ledger_sequence: number;
  generated_at: string;
  freshness: CheckpointFreshness | null;
  privacy_policy_version: string;
  privacy_summary: Record<string, unknown>;
  budget: ContextBudget;
  included_bytes: number;
  estimated_tokens: number;
  estimator_id: string;
  estimator_uncertainty: string;
  items: ContextPackItem[];
  omissions: ContextOmission[];
  unavailable_sources: string[];
  request_fingerprint: string;
  content_fingerprint: string;
}

export interface SavedContextPack {
  pack: ContextPack;
  generated_artifact_id: string;
  created_by: string;
}

export interface CheckpointComparison {
  older_checkpoint_id: string;
  newer_checkpoint_id: string;
  older_ledger_sequence: number;
  newer_ledger_sequence: number;
  added_source_ids: string[];
  removed_source_ids: string[];
  changed_source_ids: string[];
  intervening_events: CheckpointEventDelta[];
  events_truncated: boolean;
}
