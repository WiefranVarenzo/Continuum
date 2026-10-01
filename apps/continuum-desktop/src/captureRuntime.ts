import type {
  CaptureBackpressureAction,
  CaptureBufferPolicy,
  CaptureCapability,
  CaptureSourceKind,
} from "./captureRuntimeTypes";

export const CAPTURE_BUFFER_POLICY: CaptureBufferPolicy = {
  max_buffer_bytes: 16 * 1024 * 1024,
  max_segment_bytes: 32 * 1024 * 1024,
  max_pending_segments: 4,
  quality_reduction_allowed: false,
};

export function requestedCaptureSources(selection: Record<CaptureSourceKind, boolean>): CaptureSourceKind[] {
  return (["screen", "system_audio", "microphone"] as CaptureSourceKind[]).filter((kind) => selection[kind]);
}

export function detectCaptureCapabilities(scope: typeof globalThis = globalThis): CaptureCapability[] {
  const mediaDevices = scope.navigator?.mediaDevices;
  const displayAvailable = typeof mediaDevices?.getDisplayMedia === "function";
  const microphoneAvailable = typeof mediaDevices?.getUserMedia === "function";
  return [
    capability("screen", "webview-display", displayAvailable, "Screen sharing is unavailable in this WebView."),
    capability("system_audio", "webview-display-audio", displayAvailable, "System audio sharing is unavailable in this WebView."),
    capability("microphone", "webview-microphone", microphoneAvailable, "Microphone capture is unavailable in this WebView."),
  ];
}

function capability(source_kind: CaptureSourceKind, capability_id: string, available: boolean, unavailableReason: string): CaptureCapability {
  return { source_kind, capability_id, available, reason: available ? null : unavailableReason };
}

export function chooseRecorderMimeType(hasVideo: boolean, supports: (value: string) => boolean, hasAudio = false): string {
  const candidates = hasVideo
    ? hasAudio ? ["video/webm;codecs=vp8,opus"] : ["video/webm;codecs=vp8", "video/webm"]
    : ["audio/webm;codecs=opus", "audio/webm"];
  return candidates.find(supports) ?? "";
}

export function evaluateCapturePressure(
  policy: CaptureBufferPolicy,
  bufferedBytes: number,
  pendingSegments: number,
): CaptureBackpressureAction {
  if (bufferedBytes >= policy.max_buffer_bytes || pendingSegments >= policy.max_pending_segments) {
    return policy.quality_reduction_allowed ? "reduce_quality" : "pause";
  }
  if (bufferedBytes * 100 >= policy.max_buffer_bytes * 75) return "flush_segment";
  return "continue";
}

export function observedTrackPermission(
  stream: Pick<MediaStream, "getVideoTracks" | "getAudioTracks">,
  sourceKind: CaptureSourceKind,
): "granted" | "unavailable" {
  const tracks = sourceKind === "screen" ? stream.getVideoTracks() : stream.getAudioTracks();
  return tracks.length > 0 ? "granted" : "unavailable";
}
