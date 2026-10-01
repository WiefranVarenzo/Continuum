import type { DataClassification } from "./contracts";

export type CaptureSourceKind = "screen" | "system_audio" | "microphone";
export type CapturePermissionStatus = "unknown" | "granted" | "denied" | "unavailable" | "revoked";
export type CaptureSessionState =
  | "awaiting_permission"
  | "ready"
  | "blocked"
  | "capturing"
  | "paused"
  | "finalizing"
  | "completed"
  | "interrupted"
  | "failed"
  | "cancelled";
export type CaptureExternalKind = "browser" | "file" | "web" | "screenshot";

export interface CaptureCapability {
  source_kind: CaptureSourceKind;
  capability_id: string;
  available: boolean;
  reason: string | null;
}

export interface CaptureSourceRequest {
  source_kind: CaptureSourceKind;
  capability_id: string;
}

export interface CaptureBackendDescriptor {
  backend_id: string;
  backend_version: string;
  platform: string;
  capabilities: CaptureCapability[];
}

export interface CaptureEncodingSettings {
  container: string;
  video_codec: string | null;
  audio_codec: string | null;
  width: number | null;
  height: number | null;
  frames_per_second: number | null;
  sample_rate_hz: number | null;
  channels: number | null;
  segment_duration_ms: number;
}

export interface CaptureBufferPolicy {
  max_buffer_bytes: number;
  max_segment_bytes: number;
  max_pending_segments: number;
  quality_reduction_allowed: boolean;
}

export interface NewCaptureSession {
  research_session_id: string | null;
  sources: CaptureSourceRequest[];
  backend: CaptureBackendDescriptor;
  encoding: CaptureEncodingSettings;
  buffer_policy: CaptureBufferPolicy;
  legal_consent_acknowledged: boolean;
}

export interface CaptureSessionSource {
  source_kind: CaptureSourceKind;
  capability_id: string;
  permission_status: CapturePermissionStatus;
  permission_checked_at: string | null;
  permission_reference: string | null;
}

export interface CaptureSession {
  id: string;
  project_id: string;
  research_session_id: string | null;
  state: CaptureSessionState;
  state_version: number;
  backend: CaptureBackendDescriptor;
  encoding: CaptureEncodingSettings;
  buffer_policy: CaptureBufferPolicy;
  legal_consent_acknowledged: boolean;
  indicator_id: string | null;
  recoverable: boolean;
  failure_code: string | null;
  failure_message: string | null;
  started_at: string | null;
  paused_at: string | null;
  ended_at: string | null;
  created_at: string;
  created_by: string;
  updated_at: string;
  updated_by: string;
  sources: CaptureSessionSource[];
}

export interface NewCaptureSegment {
  sequence: number;
  media_type: string;
  source_kinds: CaptureSourceKind[];
  start_offset_ms: number;
  end_offset_ms: number;
  complete: boolean;
  recovery_note: string | null;
}

export interface CaptureSegment {
  id: string;
  project_id: string;
  capture_session_id: string;
  sequence: number;
  state: string;
  artifact_id: string;
  media_type: string;
  source_kinds: CaptureSourceKind[];
  start_offset_ms: number;
  end_offset_ms: number;
  byte_size: number;
  sha256: string;
  recovery_note: string | null;
  created_at: string;
  created_by: string;
}

export interface NewCaptureMarker {
  segment_id: string | null;
  offset_ms: number;
  label: string;
  note: string;
}

export interface CaptureMarker extends NewCaptureMarker {
  id: string;
  project_id: string;
  capture_session_id: string;
  created_at: string;
  created_by: string;
}

export interface CapturePreview {
  artifact_id: string;
  media_type: string;
  bytes: number[];
}

export interface CaptureExternalEvidence {
  kind: CaptureExternalKind;
  title: string;
  source_uri: string | null;
  source_title: string | null;
  captured_at: string | null;
  media_type: string | null;
  bytes: number[] | null;
  source_content: string | null;
  annotation: string;
  summary: string;
  relevance: string;
  research_session_id: string | null;
  research_question_id: string | null;
  capture_session_id: string | null;
  classification: DataClassification;
  metadata: Record<string, unknown>;
}

export interface ResearchItem {
  entity: { id: string; title: string; entity_type: string };
  details: Record<string, unknown>;
}

export interface ResearchSearchHit {
  entity_id: string;
  entity_type: string;
  title: string;
  status: string;
  snippet: string;
}
