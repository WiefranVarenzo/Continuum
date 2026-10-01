import { invoke } from "@tauri-apps/api/core";
import type { HumanDocument } from "./contracts";
import type {
  CaptureExternalEvidence,
  CapturePermissionStatus,
  CaptureMarker,
  CapturePreview,
  CaptureSegment,
  CaptureSession,
  CaptureSourceKind,
  NewCaptureMarker,
  NewCaptureSegment,
  NewCaptureSession,
  ResearchItem,
  ResearchSearchHit,
} from "./captureContracts";
import type {
  CheckpointComparison,
  CheckpointEnvelope,
  CheckpointScope,
  CheckpointTrigger,
  ContextPack,
  ContextPackRequest,
  CurrentProjectState,
  SavedContextPack,
  SpaceCapabilities,
} from "./contextContracts";
import type {
  CreatedMcpGrant,
  ExternalProposal,
  ExternalProposalSummary,
  McpAuditRecord,
  McpClientGrant,
  NewMcpClientGrant,
} from "./mcpContracts";
import type {
  CreateProjectInput,
  OpenedProject,
  RepositorySyncSummary,
  ReleaseDiagnostics,
  RestoreProjectInput,
  RestoreRemoteInput,
  ProjectRemoteStatus,
  ProjectPublishResult,
  GithubStatus,
  GithubLoginProgress,
  GithubRepository,
  WorkspaceQuestionInput,
} from "./releaseContracts";

export function isDesktopRuntime(): boolean {
  return "__TAURI_INTERNALS__" in window;
}

export async function openAndCompose(path: string): Promise<HumanDocument> {
  await invoke("open_project", { path });
  return composeCurrentDocument();
}

export async function openProject(path: string): Promise<OpenedProject> {
  return invoke("open_project", { path });
}

export async function composeCurrentDocument(): Promise<HumanDocument> {
  const capabilities = await getSpaceCapabilities();
  const kind = capabilities.research && capabilities.development
    ? "integrated_report"
    : capabilities.research
      ? "research_report"
      : capabilities.development
        ? "development_report"
        : "integrated_report";
  return invoke<HumanDocument>("compose_human_document", {
    request: {
      kind,
      title: null,
      audience: "local_project",
      checkpoint_id: null,
      root_entity_ids: [],
      accepted_ai_candidate_ids: [],
      max_records: 1000,
      max_graph_nodes: 150,
      max_graph_edges: 400,
    },
  });
}

export async function createProject(input: CreateProjectInput): Promise<OpenedProject> {
  return invoke("create_project", {
    parentPath: input.parentPath,
    directoryName: input.directoryName,
    name: input.name,
    research: input.research,
    development: input.development,
  });
}

export async function restoreProject(input: RestoreProjectInput): Promise<OpenedProject> {
  return invoke("restore_project", {
    sourcePath: input.sourcePath,
    destinationParent: input.destinationParent,
    directoryName: input.directoryName,
  });
}

export async function restoreProjectFromRemote(input: RestoreRemoteInput): Promise<OpenedProject> {
  return invoke("restore_project_from_remote", {
    remoteUrl: input.remoteUrl,
    branch: input.branch || null,
    destinationParent: input.destinationParent,
    directoryName: input.directoryName,
  });
}

export async function getProjectRemoteStatus(): Promise<ProjectRemoteStatus> {
  return invoke("project_remote_status");
}

export async function publishProjectSnapshot(remoteUrl: string, branch: string | null, privateConfirmed: boolean): Promise<ProjectPublishResult> {
  return invoke("publish_project_snapshot", { remoteUrl, branch, privateConfirmed });
}

export async function getGithubStatus(): Promise<GithubStatus> {
  return invoke("github_status");
}

export async function beginGithubLogin(): Promise<GithubLoginProgress> {
  return invoke("github_begin_login");
}

export async function getGithubLoginProgress(): Promise<GithubLoginProgress> {
  return invoke("github_login_progress");
}

export async function openGithubAuthorizationPage(): Promise<void> {
  return invoke("github_open_authorization_page");
}

export async function listGithubRepositories(): Promise<GithubRepository[]> {
  return invoke("github_list_repositories");
}

export async function listGithubBranches(fullName: string): Promise<string[]> {
  return invoke("github_list_branches", { fullName });
}

export async function openGithubNewRepository(): Promise<void> {
  return invoke("github_open_new_repository");
}

export async function closeProject(): Promise<void> {
  return invoke("close_project");
}

export async function createWorkspaceQuestion(input: WorkspaceQuestionInput): Promise<void> {
  await invoke("create_workspace_question", {
    title: input.title,
    question: input.question,
    context: input.context,
    desiredOutcome: input.desiredOutcome,
    priority: input.priority,
  });
}

export async function attachAndSyncRepository(path: string): Promise<RepositorySyncSummary> {
  return invoke("attach_and_sync_repository", { path });
}

export async function runReleaseDiagnostics(): Promise<ReleaseDiagnostics> {
  return invoke("run_release_diagnostics");
}

export async function backupProject(destinationParent: string): Promise<string> {
  const result = await invoke<{ path: string }>("backup_project", { destinationParent });
  return result.path;
}

export async function exportProject(destinationParent: string): Promise<string> {
  const result = await invoke<{ path: string }>("export_project", { destinationParent });
  return result.path;
}

export async function chooseDirectory(title: string): Promise<string | null> {
  if (!isDesktopRuntime()) return null;
  const { open } = await import("@tauri-apps/plugin-dialog");
  const selected = await open({ directory: true, multiple: false, title });
  return typeof selected === "string" ? selected : null;
}

export async function saveDocument(document: HumanDocument): Promise<HumanDocument> {
  return invoke<HumanDocument>("save_human_document", { document });
}

export async function publishDocument(documentId: string, format: "html" | "markdown"): Promise<{ artifact_id: string; file_path: string }> {
  return invoke("publish_human_document", { documentId, format });
}

export async function openExportedDocument(filePath: string): Promise<void> {
  return invoke("open_exported_document", { filePath });
}

export async function createCaptureSession(request: NewCaptureSession): Promise<CaptureSession> {
  return invoke("create_capture_session", { request });
}

export async function getCaptureSession(captureSessionId: string): Promise<CaptureSession> {
  return invoke("get_capture_session", { captureSessionId });
}

export async function recordCapturePermission(
  captureSessionId: string,
  sourceKind: CaptureSourceKind,
  status: CapturePermissionStatus,
  permissionReference: string | null,
  expectedVersion: number,
): Promise<CaptureSession> {
  return invoke("record_capture_permission", {
    captureSessionId,
    sourceKind,
    status,
    permissionReference,
    expectedVersion,
  });
}

export async function beginCapture(captureSessionId: string, expectedVersion: number): Promise<CaptureSession> {
  return invoke("begin_capture", { captureSessionId, indicatorId: "capture-status-banner", expectedVersion });
}

export async function pauseCapture(captureSessionId: string, expectedVersion: number): Promise<CaptureSession> {
  return invoke("pause_capture", { captureSessionId, expectedVersion });
}

export async function resumeCapture(captureSessionId: string, expectedVersion: number): Promise<CaptureSession> {
  return invoke("resume_capture", { captureSessionId, indicatorId: "capture-status-banner", expectedVersion });
}

export async function stopCapture(captureSessionId: string, expectedVersion: number): Promise<CaptureSession> {
  return invoke("stop_capture", { captureSessionId, expectedVersion });
}

export async function interruptCapture(captureSessionId: string, reason: string, expectedVersion: number): Promise<CaptureSession> {
  return invoke("interrupt_capture", { captureSessionId, reason, expectedVersion });
}

export async function failCapture(
  captureSessionId: string,
  failureCode: string,
  failureMessage: string,
  recoverable: boolean,
  expectedVersion: number,
): Promise<CaptureSession> {
  return invoke("fail_capture", { captureSessionId, failureCode, failureMessage, recoverable, expectedVersion });
}

export async function ingestCaptureSegment(
  captureSessionId: string,
  segment: NewCaptureSegment,
  bytes: Uint8Array,
): Promise<void> {
  await invoke("ingest_capture_segment", {
    captureSessionId,
    segment,
    bytes: Array.from(bytes),
    classification: "internal",
  });
}

export async function addCaptureMarker(captureSessionId: string, marker: NewCaptureMarker): Promise<CaptureMarker> {
  return invoke("add_capture_marker", { captureSessionId, marker });
}

export async function listCaptureSegments(captureSessionId: string): Promise<CaptureSegment[]> {
  return invoke("list_capture_segments", { captureSessionId });
}

export async function listCaptureMarkers(captureSessionId: string): Promise<CaptureMarker[]> {
  return invoke("list_capture_markers", { captureSessionId });
}

export async function getCapturePreview(artifactId: string): Promise<CapturePreview> {
  return invoke("get_capture_preview", { artifactId });
}

export async function promoteCaptureSegment(segment: CaptureSegment): Promise<ResearchItem> {
  return invoke("promote_capture_segment_to_evidence", {
    request: {
      segment_id: segment.id,
      start_offset_ms: segment.start_offset_ms,
      end_offset_ms: segment.end_offset_ms,
      title: `Captured segment ${segment.sequence + 1}`,
      annotation: "Selected from the capture review panel.",
      summary: "A reviewed recording segment.",
      relevance: "User selected this segment as research Evidence.",
      question_id: null,
    },
  });
}

export async function promoteCaptureMarker(marker: CaptureMarker): Promise<ResearchItem> {
  return invoke("promote_capture_marker_to_evidence", {
    markerId: marker.id,
    title: marker.label,
    summary: marker.note || "A reviewed timestamped capture marker.",
    relevance: "User selected this marker as research Evidence.",
    questionId: null,
  });
}

export async function captureExternalEvidence(evidence: CaptureExternalEvidence): Promise<ResearchItem> {
  const item = await invoke<ResearchItem>("capture_external_evidence", { evidence });
  window.dispatchEvent(new Event("continuum:changed"));
  return item;
}

export async function listActiveResearchSessions(): Promise<ResearchSearchHit[]> {
  const page = await invoke<{ items: ResearchSearchHit[] }>("list_active_research_sessions");
  return page.items;
}

export async function researchSpaceEnabled(): Promise<boolean> {
  return invoke("research_space_enabled");
}

export async function getSpaceCapabilities(): Promise<SpaceCapabilities> {
  return invoke("space_capabilities");
}

export async function getCurrentProjectState(
  scope: CheckpointScope,
  checkpointId: string | null = null,
): Promise<CurrentProjectState> {
  return invoke("current_project_state", { scope, checkpointId });
}

export async function listCheckpoints(scope: CheckpointScope | null = null): Promise<CheckpointEnvelope[]> {
  const page = await invoke<{ items: CheckpointEnvelope[] }>("list_context_checkpoints", { scope });
  return page.items;
}

export async function createCheckpoint(input: {
  scope: CheckpointScope;
  trigger: CheckpointTrigger;
  note: string;
  blockers: string[];
  risks: string[];
  next_actions: string[];
  semantic_candidate_id: string | null;
  supersedes_checkpoint_id: string | null;
}): Promise<CheckpointEnvelope> {
  return invoke("create_context_checkpoint", { input });
}

export async function compareCheckpoints(
  firstCheckpointId: string,
  secondCheckpointId: string,
): Promise<CheckpointComparison> {
  return invoke("compare_context_checkpoints", { firstCheckpointId, secondCheckpointId });
}

export async function previewContextPack(request: ContextPackRequest): Promise<ContextPack> {
  return invoke("build_context_pack", { request });
}

export async function saveContextPack(pack: ContextPack): Promise<SavedContextPack> {
  return invoke("save_context_pack", { pack });
}

export async function listSavedContextPacks(): Promise<SavedContextPack["pack"][]> {
  const page = await invoke<{ items: Array<{ id: string }> }>("list_saved_context_packs");
  return Promise.all(page.items.map(({ id }) => invoke<SavedContextPack>("get_saved_context_pack", { contextPackId: id }).then((saved) => saved.pack)));
}

export async function createMcpClientGrant(input: NewMcpClientGrant): Promise<CreatedMcpGrant> {
  return invoke("create_mcp_client_grant", { input });
}

export async function listMcpClientGrants(): Promise<McpClientGrant[]> {
  const page = await invoke<{ items: McpClientGrant[] }>("list_mcp_client_grants");
  return page.items;
}

export async function revokeMcpClientGrant(grantId: string): Promise<McpClientGrant> {
  return invoke("revoke_mcp_client_grant", { grantId });
}

export async function listExternalProposals(status: ExternalProposalSummary["status"] | null = null): Promise<ExternalProposalSummary[]> {
  const page = await invoke<{ items: ExternalProposalSummary[] }>("list_external_proposals", { status });
  return page.items;
}

export async function getExternalProposal(proposalId: string): Promise<ExternalProposal> {
  return invoke("get_external_proposal", { proposalId });
}

export async function reviewExternalProposal(
  proposalId: string,
  expectedVersion: number,
  decision: "accept" | "reject",
  reviewNote: string,
): Promise<ExternalProposal> {
  return invoke("review_external_proposal", { proposalId, expectedVersion, decision, reviewNote });
}

export async function listMcpAudit(): Promise<McpAuditRecord[]> {
  const page = await invoke<{ items: McpAuditRecord[] }>("list_mcp_audit");
  return page.items;
}
