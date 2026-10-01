export type DataClassification = "public" | "internal" | "sensitive" | "secret" | "never_send";
export type ContributionKind = "deterministic" | "ai_assisted" | "user_authored" | "imported";

export interface DocumentContribution {
  kind: ContributionKind;
  actor_id?: string | null;
  provider_profile_id?: string | null;
  attempt_id?: string | null;
  candidate_id?: string | null;
}

export interface BlockMeta {
  id: string;
  title: string;
  source_ids: string[];
  contribution: DocumentContribution;
}

export interface DiagramNode {
  id: string;
  label: string;
  kind: string;
  status: string;
  source_ids: string[];
  preview_data_uri?: string | null;
}

export interface DiagramEdge {
  id: string;
  source: string;
  target: string;
  label: string;
  source_ids: string[];
}

export interface GraphSpecification {
  renderer: "react_flow_elk";
  layout: "org.eclipse.elk.layered";
  direction: "top_down" | "left_right";
  nodes: DiagramNode[];
  edges: DiagramEdge[];
  truncated: boolean;
  textual_alternative: string;
}

export interface MermaidSpecification {
  diagram_kind: "flowchart";
  direction: "top_down" | "left_right";
  nodes: DiagramNode[];
  edges: DiagramEdge[];
  textual_alternative: string;
}

export interface Citation {
  source_kind: string;
  source_id: string;
  source_type: string;
  title: string;
  status: string;
  origin: string;
  version: string;
  classification: DataClassification;
}

export type BlockContent =
  | { type: "overview"; summary: string; status: string; blockers: string[]; next_actions: string[] }
  | { type: "status_cards"; cards: Array<{ label: string; value: string; state: string; source_ids: string[] }> }
  | { type: "heading"; level: number }
  | { type: "prose"; text: string }
  | { type: "callout"; tone: string; text: string }
  | { type: "research_synthesis"; summary: string; key_points: string[]; limitations: string[]; recommendations: string[] }
  | { type: "key_value"; facts: Array<{ key: string; value: string; source_ids: string[] }> }
  | { type: "table"; columns: Array<{ key: string; label: string }>; rows: Array<{ id: string; cells: Record<string, string>; source_ids: string[] }>; truncated: boolean }
  | { type: "timeline"; entries: Array<{ id: string; occurred_at: string; label: string; detail: string; state: string; source_ids: string[] }>; truncated: boolean }
  | { type: "citation_list"; citation_ids: string[] }
  | { type: "artifact_reference"; artifact_id: string; media_type: string; availability: string; description: string }
  | { type: "mermaid_diagram"; specification: MermaidSpecification }
  | { type: "graph"; specification: GraphSpecification }
  | { type: "detail_group"; summary: string; details: string[] };

export interface HumanDocumentBlock {
  meta: BlockMeta;
  content: BlockContent;
}

export interface HumanDocument {
  id: string;
  schema_version: number;
  project_id: string;
  kind: string;
  title: string;
  audience: string;
  source_checkpoint_id?: string | null;
  source_ledger_sequence: number;
  generated_at: string;
  classification: DataClassification;
  omissions: string[];
  limitations: string[];
  citations: Citation[];
  blocks: HumanDocumentBlock[];
  material_fingerprint: string;
}
