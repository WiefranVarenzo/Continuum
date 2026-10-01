import { invoke } from "@tauri-apps/api/core";
import { readImage, writeText } from "@tauri-apps/plugin-clipboard-manager";
import type { ResearchItem } from "./captureContracts";
import type { ContextPackRequest } from "./contextContracts";

export interface WorkspaceItem extends ResearchItem {
  entity: ResearchItem["entity"] & { version: number; status: string; updated_at: string; metadata: Record<string, unknown> };
}
export interface ClientDetection { family: "codex" | "claude_code" | "gemini_cli" | "hermes"; name: string; executable: string }
export interface AiOrganizerResult { client_name: string; evidence_count: number; summary: string; proposal_count: number }
export function startResearch(input: { requestId: string; title: string; question: string; context: string; desiredOutcome: string }): Promise<WorkspaceItem> {
  return invoke("start_research_workspace", input);
}
export function researchItems(sessionId: string | null, sessionsOnly = false, offset = 0): Promise<{ items: WorkspaceItem[]; next_offset: number | null }> {
  return invoke("workspace_research_items", { sessionId, sessionsOnly, offset });
}
export function archiveEvidence(item: WorkspaceItem): Promise<WorkspaceItem> {
  return invoke("archive_workspace_evidence", { entityId: item.entity.id, expectedVersion: item.entity.version });
}
export const detectClients = (): Promise<ClientDetection[]> => invoke("detect_ai_clients");
export const runAiOrganizer = (sessionId: string): Promise<AiOrganizerResult> => invoke("run_ai_organizer", { sessionId });
export const exportContextMarkdown = (request: ContextPackRequest): Promise<string> => invoke("export_workspace_context", { request });
export const setCaptureDock = (compact: boolean): Promise<void> => invoke("set_capture_dock", { compact });
export const annotateEvidence = (item: WorkspaceItem, note: string): Promise<WorkspaceItem> => invoke("annotate_workspace_evidence", { entityId: item.entity.id, note, expectedVersion: item.entity.version });
export const updateEvidence = (item: WorkspaceItem, title: string, note: string): Promise<WorkspaceItem> => invoke("update_workspace_evidence", { entityId: item.entity.id, title, note, expectedVersion: item.entity.version });
export const updateResearchSession = (item: WorkspaceItem, title: string, objective: string): Promise<WorkspaceItem> => invoke("update_workspace_research_session", {
  entityId: item.entity.id,
  update: { title, objective, metadata: item.entity.metadata, expected_version: item.entity.version },
});
export const updateResearchQuestion = (item: WorkspaceItem, title: string, question: string): Promise<WorkspaceItem> => invoke("update_workspace_research_question", {
  entityId: item.entity.id,
  update: {
    title,
    kind: item.details.kind || "question",
    question,
    context: item.details.context || "",
    desired_outcome: item.details.desired_outcome || "",
    priority: item.details.priority || 2,
    due_at: item.details.due_at || null,
    metadata: item.entity.metadata,
    expected_version: item.entity.version,
  },
});
export const copyText = (text: string): Promise<void> => writeText(text);

// Only called during an explicitly enabled live session or a Paste button click.
export async function clipboardPng(): Promise<Blob | null> {
  let image;
  try { image = await readImage(); } catch (cause) {
    const message = String(cause).toLowerCase();
    if (/unavailable|not available|no image|does not contain|content not/.test(message)) return null;
    throw cause;
  }
  try {
    const { width, height } = await image.size();
    if (!width || !height || width * height > 16_000_000) throw new Error("Screenshot is too large (maximum 16 million pixels).");
    const rgba = await image.rgba();
    const canvas = document.createElement("canvas");
    canvas.width = width; canvas.height = height;
    canvas.getContext("2d")!.putImageData(new ImageData(new Uint8ClampedArray(rgba), width, height), 0, 0);
    return await new Promise<Blob>((resolve, reject) => canvas.toBlob(blob => blob ? resolve(blob) : reject(new Error("Could not encode screenshot")), "image/png"));
  } finally { await image.close(); }
}

export async function imageFingerprint(blob: Blob): Promise<string> {
  const digest = await crypto.subtle.digest("SHA-256", await blob.arrayBuffer());
  return Array.from(new Uint8Array(digest), n => n.toString(16).padStart(2, "0")).join("");
}
