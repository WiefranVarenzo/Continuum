import { useEffect, useRef, useState, type ChangeEvent } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  addCaptureMarker,
  beginCapture,
  captureExternalEvidence,
  createCaptureSession,
  failCapture,
  getCapturePreview,
  getCaptureSession,
  ingestCaptureSegment,
  interruptCapture,
  listActiveResearchSessions,
  listCaptureMarkers,
  listCaptureSegments,
  pauseCapture,
  recordCapturePermission,
  promoteCaptureMarker,
  resumeCapture,
  stopCapture,
} from "./bridge";
import type {
  CaptureExternalKind,
  CaptureMarker,
  CaptureSegment,
  CaptureSession,
  CaptureSourceKind,
  NewCaptureSession,
  ResearchSearchHit,
} from "./captureContracts";
import {
  CAPTURE_BUFFER_POLICY,
  chooseRecorderMimeType,
  detectCaptureCapabilities,
  evaluateCapturePressure,
  observedTrackPermission,
  requestedCaptureSources,
} from "./captureRuntime";
import "./capture.css";
import {captureError, recordingStream, relayScreen, requestMediaPermission, startRecorder, stopStream} from "./captureMedia";
import {desktopAudio} from "./systemAudio";

type CaptureUiState = "idle" | "requesting" | "preparing" | "capturing" | "paused" | "finalizing" | "completed" | "error";

interface CaptureRuntime {
  session: CaptureSession;
  recorder: MediaRecorder;
  streams: MediaStream[];
  sourceKinds: CaptureSourceKind[];
  sequence: number;
  lastOffsetMs: number;
  startedAt: number;
  pendingBytes: number;
  pendingSegments: number;
  queue: Promise<void>;
  stopping: boolean;
  pressurePausing: boolean;
  persistenceError: string | null;
  releaseAudio: () => void;
}

interface Props {
  projectId?: string;
  projectOpen: boolean;
  researchEnabled: boolean;
  activeResearchSessionId?: string;
  onRecordingChange?: (active: boolean) => void;
  onCaptureStatusChange?: (label: string) => void;
}

const initialSources: Record<CaptureSourceKind, boolean> = {
  screen: true,
  system_audio: false,
  microphone: false,
};

interface CaptureRuntimeHealth {
  platform?: string;
  recording_backend_available?: boolean | null;
  missing_recording_elements?: string[];
  native_screen_available?: boolean;
  missing_native_screen_elements?: string[];
  native_screen_audio_available?: boolean;
  missing_native_screen_audio_elements?: string[];
  native_system_audio_available?: boolean;
  scanner_ready: boolean;
  pipewire_socket_ready: boolean;
  desktop_portal_session_ready: boolean;
  summary: string;
}

export function CapturePanel({ projectId, projectOpen, researchEnabled, activeResearchSessionId, onRecordingChange, onCaptureStatusChange }: Props) {
  const [selection, setSelection] = useState(initialSources);
  const [consent, setConsent] = useState(false);
  const [uiState, setUiState] = useState<CaptureUiState>("idle");
  const [session, setSession] = useState<CaptureSession | null>(null);
  const [error, setError] = useState("");
  const [notice, setNotice] = useState("");
  const [markerLabel, setMarkerLabel] = useState("Research bookmark");
  const [sourceUrl, setSourceUrl] = useState("");
  const [sourceTitle, setSourceTitle] = useState("");
  const [sourceExcerpt, setSourceExcerpt] = useState("");
  const [sourceKind, setSourceKind] = useState<Extract<CaptureExternalKind, "browser" | "web">>("web");
  const [researchSessions, setResearchSessions] = useState<ResearchSearchHit[]>([]);
  const [researchSessionId, setResearchSessionId] = useState("");
  const [capturedSegments, setCapturedSegments] = useState<CaptureSegment[]>([]);
  const [capturedMarkers, setCapturedMarkers] = useState<CaptureMarker[]>([]);
  const [preview, setPreview] = useState<{ url: string; mediaType: string } | null>(null);
  const [runtimeHealth, setRuntimeHealth] = useState<CaptureRuntimeHealth | null>(null);
  const [recordingTitle,setRecordingTitle]=useState("");
  const [recordingNote,setRecordingNote]=useState("");
  const [savingRecording,setSavingRecording]=useState(false);
  const [savedRecording,setSavedRecording]=useState(false);
  const [previousRecordings,setPreviousRecordings]=useState<CaptureSession[]>([]);
  const [nativeScreenActive,setNativeScreenActive]=useState(false);
  const [nativeSavePending,setNativeSavePending]=useState(false);
  const runtime = useRef<CaptureRuntime | null>(null);
  const permissionRequest = useRef<AbortController | null>(null);
  const nativeScreenStarted = useRef(false);
  const nativeStopTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const nativeMetadata = useRef({ title: "", annotation: "", researchSessionId: "" });
  nativeMetadata.current = { title: recordingTitle, annotation: recordingNote, researchSessionId };

  const visibleIndicator = ["requesting", "preparing", "capturing", "paused", "finalizing"].includes(uiState);
  const captureAvailable = projectOpen && researchEnabled;
  const canStart = captureAvailable && consent && requestedCaptureSources(selection).length > 0 && !visibleIndicator && !nativeSavePending;

  useEffect(() => { onRecordingChange?.(visibleIndicator); }, [visibleIndicator, onRecordingChange]);
  useEffect(() => { onCaptureStatusChange?.(uiState === "requesting" ? "Waiting for permission" : uiState === "preparing" ? "Preparing capture" : uiState === "capturing" ? "Recording" : uiState === "paused" ? "Capture paused" : uiState === "finalizing" ? "Saving recording" : ""); }, [uiState, onCaptureStatusChange]);
  useEffect(() => { if (activeResearchSessionId && !visibleIndicator) setResearchSessionId(activeResearchSessionId); }, [activeResearchSessionId, visibleIndicator]);

  useEffect(() => () => {
    permissionRequest.current?.abort();
    if (nativeStopTimer.current) clearTimeout(nativeStopTimer.current);
    if (nativeScreenStarted.current) {
      // Preserve an in-progress recording on navigation rather than silently
      // discarding its media. The backend will close the portal after saving.
      void invoke("stop_native_screen_capture", {
        title: "Screen recording",
        annotation: "Saved automatically when leaving the capture page.",
        researchSessionId: null,
      }).then(() => window.dispatchEvent(new Event("continuum:changed"))).catch(() => {
        void invoke("cancel_native_screen_capture").catch(() => undefined);
      });
      nativeScreenStarted.current = false;
    }
    const active = runtime.current;
    if (!active) return;
    active.stopping = true;
    active.streams.forEach(stopStream);
    active.releaseAudio();
    if (active.recorder.state !== "inactive") active.recorder.stop();
    void interruptCapture(active.session.id, "capture UI closed", active.session.state_version).catch(() => undefined);
  }, []);

  useEffect(() => {
    if (!captureAvailable) return;
    void invoke<CaptureRuntimeHealth>("capture_runtime_health")
      .then(setRuntimeHealth)
      .catch(() => setRuntimeHealth(null));
    void listActiveResearchSessions()
      .then((items) => {
        setResearchSessions(items);
        if (activeResearchSessionId) setResearchSessionId(activeResearchSessionId);
        else if (items.length === 1) setResearchSessionId(items[0].entity_id);
      })
      .catch((cause) => setError(errorMessage(cause)));
    void invoke<{items:CaptureSession[]}>("list_capture_sessions",{limit:25,offset:0}).then(page=>setPreviousRecordings(page.items.filter(s=>s.backend.backend_id==="webview-media-recorder"))).catch(cause=>setError(errorMessage(cause)));
  }, [captureAvailable, activeResearchSessionId]);

  useEffect(() => () => {
    if (preview) URL.revokeObjectURL(preview.url);
  }, [preview]);

  function setSource(kind: CaptureSourceKind, checked: boolean) {
    setSelection((current) => ({ ...current, [kind]: checked }));
  }

  async function startRecording() {
    setError("");
    setNotice("");
    if (!captureAvailable) return setError("Enable Research Space in this project before capturing Evidence.");
    if (!consent) return setError("Acknowledge lawful capture responsibility first.");
    const sourceKinds = requestedCaptureSources(selection);
    if (sourceKinds.length === 0) return setError("Select at least one capture source.");
    // Linux keeps its verified PipeWire recorder. Windows uses WebView2's
    // user-approved sharing dialog and the segmented MediaRecorder path below.
    if (selection.screen && runtimeHealth?.platform !== "windows") {
      if (!runtimeHealth?.native_screen_available) {
        return setError(`Native screen recording is unavailable: ${runtimeHealth?.missing_native_screen_elements?.join(", ") || runtimeHealth?.summary || "recording components not ready"}. The older WebView path is disabled because it produced black video.`);
      }
      if ((selection.system_audio || selection.microphone) && !runtimeHealth.native_screen_audio_available) {
        return setError(`Screen audio recording is unavailable: ${runtimeHealth.missing_native_screen_audio_elements?.join(", ") || "audio components not ready"}. Nothing was captured.`);
      }
      setUiState("requesting");
      try {
        await invoke("start_native_screen_capture", {
          systemAudio: selection.system_audio,
          microphone: selection.microphone,
          consent,
        });
        nativeScreenStarted.current = true;
        setNativeScreenActive(true);
        setNativeSavePending(false);
        setRecordingTitle("");
        setRecordingNote("");
        setUiState("capturing");
        setNotice(`Recording Screen${selection.system_audio ? " + System audio" : ""}${selection.microphone ? " + Microphone" : ""}. Stop within one minute to save it to the workspace.`);
        nativeStopTimer.current = setTimeout(() => { void stopNativeScreenRecording(); }, 55_000);
      } catch (cause) {
        setUiState("error");
        setError(errorMessage(cause));
      }
      return;
    }
    if (typeof MediaRecorder === "undefined") return setError("MediaRecorder is unavailable in this desktop WebView.");
    if (runtimeHealth?.recording_backend_available === false) return setError(runtimeHealth.summary);
    const hasVideo = selection.screen;
    const mimeType = chooseRecorderMimeType(hasVideo, MediaRecorder.isTypeSupported.bind(MediaRecorder), selection.system_audio || selection.microphone);
    if (!mimeType) return setError("No supported WebM recording encoder was found. Screen/microphone permission cannot fix this. Use the updated Continuum package with its recording plugins.");

    const capabilities = detectCaptureCapabilities();
    const nativeAudio=!!projectId && runtimeHealth?.platform === "linux" && !!runtimeHealth.native_system_audio_available;
    if (nativeAudio) {
      const capability=capabilities.find(item=>item.source_kind==="system_audio")!;
      capability.available=true;capability.reason=null;capability.capability_id="linux-desktop-audio-monitor";
    }
    const unavailable = sourceKinds.find((kind) => !capabilities.find((item) => item.source_kind === kind)?.available);
    if (unavailable) return setError(`${sourceLabel(unavailable)} is unavailable on this system.`);

    setUiState("requesting");
    let created: CaptureSession | null = null;
    const streams: MediaStream[] = [];
    const controller = new AbortController();
    permissionRequest.current = controller;
    let releaseAudio = () => {};
    let releaseRelay = () => {};
    let releaseDesktopAudio = () => {};
    let preparedAudio:AudioContext|undefined;
    controller.signal.addEventListener("abort",()=>streams.forEach(stopStream),{once:true});
    try {
      // Unlock Web Audio in this click as well; waiting for the OS chooser can
      // otherwise leave the mixer suspended indefinitely in WebKit.
      if(selection.system_audio&&(nativeAudio||selection.microphone)) {
        preparedAudio=new AudioContext({sampleRate:48000});
        void preparedAudio.resume().catch(()=>{});
        controller.signal.addEventListener("abort",()=>{if(preparedAudio)void preparedAudio.close();},{once:true});
      }
      // Do not await persistence or IPC before getDisplayMedia: it needs this click's activation.
      let displayStream: MediaStream | null = null;
      let microphoneStream: MediaStream | null = null;
      let systemAudioStream: MediaStream | null = null;
      if (selection.screen || (selection.system_audio && !nativeAudio)) {
        displayStream = await requestMediaPermission(navigator.mediaDevices.getDisplayMedia({video:true,audio:selection.system_audio&&!nativeAudio}), "screen sharing", controller.signal);
        streams.push(displayStream);
        if (selection.system_audio && !nativeAudio && !displayStream.getAudioTracks().length) {
          throw new Error("Your screen-sharing backend returned video without system audio. Turn off System audio to record the screen, or select Microphone to record your voice. The requested combination was not recorded.");
        }
      }
      let relayedScreen:MediaStream|null=null;
      if(selection.screen && displayStream){const relay=await relayScreen(displayStream,controller.signal);relayedScreen=relay.stream;releaseRelay=relay.release;}
      if (selection.microphone) {
        microphoneStream = await requestMediaPermission(navigator.mediaDevices.getUserMedia({audio:true,video:false}), "microphone", controller.signal);
        streams.push(microphoneStream);
      }
      if (selection.system_audio && nativeAudio) {
        const desktop=await desktopAudio(projectId!,controller.signal,error=>{
          const active=runtime.current;
          if(active)void failActiveCapture(active,"desktop_audio_stopped",error.message);
          else {controller.abort();setError(error.message);}
        },preparedAudio);
        systemAudioStream=desktop.stream;releaseDesktopAudio=desktop.release;streams.push(desktop.stream);
      } else if (selection.system_audio) systemAudioStream=displayStream;
      if (controller.signal.aborted) throw new DOMException("Capture cancelled.", "AbortError");
      setUiState("preparing");
      const request: NewCaptureSession = {
        research_session_id: researchSessionId || null,
        sources: sourceKinds.map((source_kind) => ({
          source_kind,
          capability_id: capabilities.find((item) => item.source_kind === source_kind)!.capability_id,
        })),
        backend: {
          backend_id: "webview-media-recorder",
          backend_version: "1",
          platform: navigator.userAgent.slice(0, 100),
          capabilities,
        },
        encoding: {
          container: "webm",
          video_codec: hasVideo ? "vp8" : null,
          audio_codec: selection.system_audio || selection.microphone ? "opus" : null,
          width: null,
          height: null,
          frames_per_second: hasVideo ? 30 : null,
          sample_rate_hz: selection.system_audio || selection.microphone ? 48_000 : null,
          channels: selection.system_audio || selection.microphone ? 2 : null,
          segment_duration_ms: 5_000,
        },
        buffer_policy: CAPTURE_BUFFER_POLICY,
        legal_consent_acknowledged: true,
      };
      created = await createCaptureSession(request);
      setSession(created);

      if (displayStream) {
          if (selection.screen) {
            created = await observePermission(created, "screen", observedTrackPermission(displayStream, "screen"));
          }
      }
      if (systemAudioStream) created = await observePermission(created,"system_audio",observedTrackPermission(systemAudioStream,"system_audio"));
      if (microphoneStream) {
          created = await observePermission(created, "microphone", observedTrackPermission(microphoneStream, "microphone"));
      }

      if (created.state !== "ready") throw new Error("One or more requested capture sources were not granted.");
      const tracks: MediaStreamTrack[] = [];
      if (selection.screen) tracks.push(...(relayedScreen?.getVideoTracks() ?? []));
      if (selection.system_audio) tracks.push(...(systemAudioStream?.getAudioTracks() ?? []));
      if (selection.microphone) tracks.push(...(microphoneStream?.getAudioTracks() ?? []));
      if (tracks.length === 0) throw new Error("The operating system returned no usable media tracks.");

      const mixed = await recordingStream(tracks,preparedAudio);
      const mediaStream = mixed.stream;
      releaseAudio = mixed.release;
      if (controller.signal.aborted) throw new DOMException("Capture cancelled.", "AbortError");
      const recorder = new MediaRecorder(mediaStream, { mimeType, videoBitsPerSecond: hasVideo ? 2_500_000 : undefined, audioBitsPerSecond: 128_000 });
      created = await beginCapture(created.id, created.state_version);
      if(controller.signal.aborted || tracks.some(track=>track.readyState!=="live"))throw new DOMException("Capture cancelled or device stopped before recording began.","AbortError");
      const active: CaptureRuntime = {
        session: created,
        recorder,
        streams,
        sourceKinds,
        sequence: 0,
        lastOffsetMs: 0,
        startedAt: performance.now(),
        pendingBytes: 0,
        pendingSegments: 0,
        queue: Promise.resolve(),
        stopping: false,
        pressurePausing: false,
        persistenceError: null,
        releaseAudio:()=>{releaseAudio();releaseRelay();releaseDesktopAudio();if(preparedAudio)void preparedAudio.close();},
      };
      runtime.current = active;
      setSession(created);
      recorder.addEventListener("dataavailable", (event) => enqueueSegment(active, event.data));
      recorder.addEventListener("error", (event) => void failActiveCapture(active, "media_recorder_error", event.error.message));
      streams.flatMap((stream) => stream.getTracks()).forEach((track) => {
        track.addEventListener("ended", () => void interruptActiveCapture(active, `${track.kind} source ended`), { once: true });
      });
      setCapturedSegments([]);
      setCapturedMarkers([]);
      await startRecorder(recorder, controller.signal);
      if (controller.signal.aborted || runtime.current !== active || active.stopping) throw new Error("Capture stopped while the recorder was preparing.");
      setUiState("capturing");
      setNotice("Capture started. The red indicator remains visible while sensors are active.");
      setSavedRecording(false);setRecordingTitle("");setRecordingNote("");
    } catch (cause) {
      const active = runtime.current;
      if (active && active.session.id === created?.id) {
        active.stopping = true;
        if (active.recorder.state !== "inactive") {
          await new Promise<void>(resolve => {
            const finish = () => { clearTimeout(timer); active.recorder.removeEventListener("stop", finish); resolve(); };
            const timer = setTimeout(finish, 2_000);
            active.recorder.addEventListener("stop", finish, {once:true});
            try { active.recorder.stop(); } catch { finish(); }
          });
        }
        await active.queue.catch(() => undefined);
      }
      streams.forEach(stopStream);
      releaseAudio();
      releaseRelay();
      releaseDesktopAudio();
      if(preparedAudio)void preparedAudio.close();
      if (runtime.current?.session.id === created?.id) runtime.current = null;
      if (created) {
        await safeFail(created, "capture_start_failed", errorMessage(cause), (active?.sequence ?? 0) > 0);
      }
      setUiState("error");
      setError(captureError(cause, runtimeHealth?.platform));
    } finally {
      if (permissionRequest.current === controller) permissionRequest.current = null;
    }
  }

  async function observePermission(current: CaptureSession, kind: CaptureSourceKind, status: "granted" | "denied" | "unavailable") {
    const next = await recordCapturePermission(current.id, kind, status, `webview:${status}`, current.state_version);
    setSession(next);
    return next;
  }

  function enqueueSegment(active: CaptureRuntime, blob: Blob) {
    if (blob.size === 0) return;
    const startOffset = active.lastOffsetMs;
    const endOffset = Math.max(startOffset + 1, Math.round(performance.now() - active.startedAt));
    const sequence = active.sequence++;
    active.lastOffsetMs = endOffset;
    active.pendingBytes += blob.size;
    active.pendingSegments += 1;
    const pressure = evaluateCapturePressure(CAPTURE_BUFFER_POLICY, active.pendingBytes, active.pendingSegments);
    if (blob.size > CAPTURE_BUFFER_POLICY.max_segment_bytes) {
      void failActiveCapture(active, "segment_too_large", "A capture segment exceeded the 32 MB safety limit.");
      return;
    }
    if (pressure === "pause") void pauseForPressure(active);
    active.queue = active.queue.then(async () => {
      const bytes = new Uint8Array(await blob.arrayBuffer());
      await ingestCaptureSegment(active.session.id, {
        sequence,
        media_type: blob.type || active.recorder.mimeType || (selection.screen ? "video/webm" : "audio/webm"),
        source_kinds: active.sourceKinds,
        start_offset_ms: startOffset,
        end_offset_ms: endOffset,
        complete: true,
        recovery_note: null,
      }, bytes);
    }).catch((cause) => {
      active.persistenceError=errorMessage(cause);
      void failActiveCapture(active, "segment_persist_failed", errorMessage(cause));
    }).finally(() => {
      active.pendingBytes = Math.max(0, active.pendingBytes - blob.size);
      active.pendingSegments = Math.max(0, active.pendingSegments - 1);
    });
  }

  async function pauseForPressure(active: CaptureRuntime) {
    if (active.pressurePausing || active.stopping || active.recorder.state !== "recording") return;
    active.pressurePausing = true;
    active.recorder.pause();
    try {
      active.session = await pauseCapture(active.session.id, active.session.state_version);
      setSession(active.session);
      setUiState("paused");
      setNotice("Capture paused automatically because the persistence queue reached its memory limit.");
    } catch (cause) {
      await failActiveCapture(active, "backpressure_pause_failed", errorMessage(cause));
    } finally {
      active.pressurePausing = false;
    }
  }

  async function pauseRecording() {
    const active = runtime.current;
    if (!active || active.recorder.state !== "recording") return;
    active.recorder.pause();
    try {
      active.session = await pauseCapture(active.session.id, active.session.state_version);
      setSession(active.session);
      setUiState("paused");
    } catch (cause) {
      active.recorder.resume();
      setError(errorMessage(cause));
    }
  }

  async function resumeRecording() {
    const active = runtime.current;
    if (!active || active.recorder.state !== "paused") return;
    try {
      active.session = await resumeCapture(active.session.id, active.session.state_version);
      active.recorder.resume();
      setSession(active.session);
      setUiState("capturing");
      setNotice("");
    } catch (cause) {
      setError(errorMessage(cause));
    }
  }

  async function stopRecording() {
    if (nativeScreenActive || nativeSavePending) return stopNativeScreenRecording();
    const active = runtime.current;
    if (!active || active.stopping) return;
    active.stopping = true;
    setUiState("finalizing");
    try {
      if (active.recorder.state !== "inactive") {
        const stopped = new Promise<void>((resolve) => active.recorder.addEventListener("stop", () => resolve(), { once: true }));
        active.recorder.stop();
        await stopped;
      }
      await active.queue;
      active.streams.forEach(stopStream);
      active.releaseAudio();
      if(active.persistenceError)throw new Error(`Some recording data could not be saved: ${active.persistenceError}. Saved fragments remain available.`);
      active.session = await getCaptureSession(active.session.id);
      active.session = await stopCapture(active.session.id, active.session.state_version);
      setSession(active.session);
      setPreviousRecordings(items=>[active.session,...items.filter(s=>s.id!==active.session.id)].slice(0,25));
      setUiState("completed");
      setNotice("Recording saved locally. Give it a title below to add the complete recording to your workspace.");
      runtime.current = null;
      try {
        await loadReview(active.session.id);
      } catch (cause) {
        setError(`Capture completed, but its review list could not be loaded: ${errorMessage(cause)}`);
      }
    } catch (cause) {
      active.streams.forEach(stopStream);
      active.releaseAudio();
      await safeFail(active.session, "capture_finalize_failed", errorMessage(cause), true);
      setUiState("error");
      setError(errorMessage(cause));
      runtime.current = null;
    }
  }

  async function stopNativeScreenRecording() {
    if (nativeStopTimer.current) { clearTimeout(nativeStopTimer.current); nativeStopTimer.current = null; }
    setUiState("finalizing");
    setError("");
    try {
      await invoke("stop_native_screen_capture", {
        title: nativeMetadata.current.title.trim() || "Screen recording",
        annotation: nativeMetadata.current.annotation.trim(),
        researchSessionId: nativeMetadata.current.researchSessionId || null,
      });
      nativeScreenStarted.current = false;
      setNativeScreenActive(false);
      setNativeSavePending(false);
      setUiState("completed");
      setNotice("Recording saved in Workspace. Open its card to play the video and check its audio.");
      window.dispatchEvent(new Event("continuum:changed"));
    } catch (cause) {
      // Only persistence failures leave a completed recording in backend
      // memory. A failed encoder/finalization has no media to retry; restore
      // Start instead of trapping the user behind an impossible Retry save.
      const message = errorMessage(cause);
      const retryable = message.includes("Recording is held in memory");
      nativeScreenStarted.current = false;
      setNativeScreenActive(false);
      setNativeSavePending(retryable);
      setUiState("error");
      setError(message);
    }
  }

  async function interruptActiveCapture(active: CaptureRuntime, reason: string) {
    if (active.stopping || runtime.current !== active) return;
    active.stopping = true;
    if (active.recorder.state !== "inactive") {
      const stopped = new Promise<void>((resolve) => active.recorder.addEventListener("stop", () => resolve(), { once: true }));
      active.recorder.stop();
      await stopped;
    }
    await active.queue.catch(() => undefined);
    active.streams.forEach(stopStream);
    active.releaseAudio();
    try {
      active.session = await getCaptureSession(active.session.id);
      if (["capturing", "paused"].includes(active.session.state)) {
        active.session = await interruptCapture(active.session.id, reason, active.session.state_version);
      }
      setSession(active.session);
    } catch (cause) {
      setError(errorMessage(cause));
    }
    setUiState("error");
    setError(`Capture interrupted: ${reason}. Saved segments remain recoverable.`);
    runtime.current = null;
  }

  async function failActiveCapture(active: CaptureRuntime, code: string, message: string) {
    if (active.stopping || runtime.current !== active) return;
    active.stopping = true;
    if (active.recorder.state !== "inactive") {
      const stopped = new Promise<void>((resolve) => active.recorder.addEventListener("stop", () => resolve(), { once: true }));
      active.recorder.stop();
      await stopped;
    }
    await active.queue.catch(() => undefined);
    active.streams.forEach(stopStream);
    active.releaseAudio();
    await safeFail(active.session, code, message, active.sequence > 0);
    setUiState("error");
    setError(message);
    runtime.current = null;
  }

  async function safeFail(current: CaptureSession, code: string, message: string, recoverable: boolean) {
    try {
      const fresh = await getCaptureSession(current.id);
      if (!["completed", "failed", "cancelled"].includes(fresh.state)) {
        const failed = await failCapture(fresh.id, code, message.slice(0, 1_000), recoverable, fresh.state_version);
        setSession(failed);
      }
    } catch {
      // The original error remains user-visible; integrity verification can surface persistence failure.
    }
  }

  async function addMarker() {
    const active = runtime.current;
    if (!active) return;
    try {
      const marker = await addCaptureMarker(active.session.id, {
        segment_id: null,
        offset_ms: Math.max(0, Math.round(performance.now() - active.startedAt)),
        label: markerLabel.trim() || "Research bookmark",
        note: "Created while capture was active.",
      });
      setCapturedMarkers((items) => [...items, marker]);
      setNotice("Bookmark saved at the current capture position.");
    } catch (cause) {
      setError(errorMessage(cause));
    }
  }

  async function captureScreenshot() {
    if (!captureAvailable || !consent) return setError("Enable Research Space and acknowledge lawful capture before taking a screenshot.");
    if (!navigator.mediaDevices?.getDisplayMedia) return setError("Screen capture is unavailable in this WebView.");
    setUiState("requesting");
    setError("");
    let stream: MediaStream | null = null;
    let screenshotSession: CaptureSession | null = null;
    const controller = new AbortController();
    permissionRequest.current = controller;
    try {
      stream = await requestMediaPermission(navigator.mediaDevices.getDisplayMedia({video:true,audio:false}), "screen sharing", controller.signal);
      const capabilities = detectCaptureCapabilities();
      screenshotSession = await createCaptureSession({
        research_session_id: researchSessionId || null,
        sources: [{ source_kind: "screen", capability_id: "webview-display" }],
        backend: {
          backend_id: "webview-screenshot",
          backend_version: "1",
          platform: navigator.userAgent.slice(0, 100),
          capabilities,
        },
        encoding: {
          container: "png",
          video_codec: null,
          audio_codec: null,
          width: null,
          height: null,
          frames_per_second: null,
          sample_rate_hz: null,
          channels: null,
          segment_duration_ms: 1_000,
        },
        buffer_policy: CAPTURE_BUFFER_POLICY,
        legal_consent_acknowledged: true,
      });
      if (controller.signal.aborted) throw new DOMException("Capture cancelled.", "AbortError");
      const screenPermission = observedTrackPermission(stream, "screen");
      screenshotSession = await recordCapturePermission(screenshotSession.id, "screen", screenPermission, `webview:${screenPermission}`, screenshotSession.state_version);
      if (screenshotSession.state !== "ready") throw new Error("The selected screen returned no usable video track.");
      screenshotSession = await beginCapture(screenshotSession.id, screenshotSession.state_version);
      setSession(screenshotSession);
      setUiState("capturing");
      const video = document.createElement("video");
      video.muted = true;
      video.srcObject = stream;
      await video.play();
      if (!video.videoWidth || !video.videoHeight) throw new Error("The selected screen returned no video frame.");
      const canvas = document.createElement("canvas");
      canvas.width = video.videoWidth;
      canvas.height = video.videoHeight;
      canvas.getContext("2d")?.drawImage(video, 0, 0);
      const blob = await new Promise<Blob>((resolve, reject) => canvas.toBlob((value) => value ? resolve(value) : reject(new Error("Could not encode screenshot.")), "image/png"));
      if (blob.size > CAPTURE_BUFFER_POLICY.max_segment_bytes) throw new Error("Screenshot exceeds the 32 MB safety limit.");
      await captureExternalEvidence({
        ...externalBase("screenshot", sourceTitle.trim() || "Captured screenshot", screenshotSession.id),
        media_type: "image/png",
        bytes: Array.from(new Uint8Array(await blob.arrayBuffer())),
        metadata: { width: canvas.width, height: canvas.height, explicit_user_action: true },
      });
      setUiState("finalizing");
      screenshotSession = await stopCapture(screenshotSession.id, screenshotSession.state_version);
      setSession(screenshotSession);
      setNotice("Screenshot stored as Evidence. Its original PNG remains unchanged.");
      setUiState("idle");
    } catch (cause) {
      if (screenshotSession) {
        try {
          const fresh = await getCaptureSession(screenshotSession.id);
          if (fresh.state === "awaiting_permission") {
            screenshotSession = await recordCapturePermission(fresh.id, "screen", "denied", "webview:denied", fresh.state_version);
          } else if (!["blocked", "completed", "failed", "cancelled"].includes(fresh.state)) {
            await safeFail(fresh, "screenshot_failed", errorMessage(cause), false);
          }
        } catch {
          // Preserve the original OS/backend error for the user.
        }
      }
      setError(captureError(cause, runtimeHealth?.platform));
      setUiState("error");
    } finally {
      if (stream) stopStream(stream);
      if (permissionRequest.current === controller) permissionRequest.current = null;
    }
  }

  async function captureFile(event: ChangeEvent<HTMLInputElement>) {
    const file = event.target.files?.[0];
    event.target.value = "";
    if (!file) return;
    if (!captureAvailable) return setError("Enable Research Space before importing research Evidence.");
    if (file.size > CAPTURE_BUFFER_POLICY.max_segment_bytes) return setError("File exceeds the 32 MB capture limit.");
    try {
      await captureExternalEvidence({
        ...externalBase("file", file.name),
        media_type: file.type || "application/octet-stream",
        bytes: Array.from(new Uint8Array(await file.arrayBuffer())),
        metadata: { file_name: file.name, last_modified: file.lastModified },
      });
      setNotice(`${file.name} stored as Evidence.`);
      setError("");
    } catch (cause) {
      setError(errorMessage(cause));
    }
  }

  async function captureWebSource() {
    if (!captureAvailable) return setError("Enable Research Space before capturing a research source.");
    if (!sourceUrl.trim() || !sourceTitle.trim() || !sourceExcerpt.trim()) return setError("URL, title, and captured excerpt are required.");
    try {
      await captureExternalEvidence({
        ...externalBase(sourceKind, sourceTitle.trim()),
        source_uri: sourceUrl.trim(),
        source_title: sourceTitle.trim(),
        source_content: sourceExcerpt,
        metadata: { explicit_user_capture: true, untrusted_external_content: true },
      });
      setNotice("Web source stored as untrusted Evidence with its capture timestamp.");
      setError("");
    } catch (cause) {
      setError(errorMessage(cause));
    }
  }

  async function loadReview(captureSessionId: string) {
    const [segments, markers] = await Promise.all([
      listCaptureSegments(captureSessionId),
      listCaptureMarkers(captureSessionId),
    ]);
    setCapturedSegments(segments);
    setCapturedMarkers(markers);
  }

  async function openRecording(item:CaptureSession){
    if(visibleIndicator)return;
    try {await loadReview(item.id);setSession(item);setUiState(item.state==="completed"?"completed":"error");setSavedRecording(false);setRecordingTitle("");setRecordingNote("");setError("");setNotice("Original recording loaded. If already added, adding it again will open the same saved Evidence, not duplicate it.");}
    catch(cause){setError(errorMessage(cause));}
  }

  async function previewSegment(segment: CaptureSegment) {
    try {
      const value = await getCapturePreview(segment.artifact_id);
      if (preview) URL.revokeObjectURL(preview.url);
      setPreview({
        url: URL.createObjectURL(new Blob([new Uint8Array(value.bytes)], { type: value.media_type })),
        mediaType: value.media_type,
      });
      setError("");
    } catch (cause) {
      setError(`${errorMessage(cause)} Preview is intentionally limited to 8 MiB; the Artifact remains stored.`);
    }
  }

  async function saveRecording() {
    if(!session||!recordingTitle.trim()||savingRecording||savedRecording)return;
    setSavingRecording(true);setError("");
    try {
      await invoke("promote_capture_recording",{captureSessionId:session.id,title:recordingTitle.trim(),annotation:recordingNote});
      setSavedRecording(true);
      window.dispatchEvent(new Event("continuum:changed"));
      setNotice("The complete recording is now in your workspace. Open its card to play it. Your title and note are available as AI context; audio/video interpretation is not automatic.");
    }catch(cause){setError(errorMessage(cause));}finally{setSavingRecording(false);}
  }

  async function promoteMarker(marker: CaptureMarker) {
    try {
      const evidence = await promoteCaptureMarker(marker);
      setNotice(`Bookmark became Evidence ${evidence.entity.id.slice(0, 8)}.`);
      setError("");
    } catch (cause) {
      setError(errorMessage(cause));
    }
  }

  function externalBase(kind: CaptureExternalKind, title: string, captureSessionId: string | null = null) {
    return {
      kind,
      title,
      source_uri: null,
      source_title: title,
      captured_at: new Date().toISOString(),
      media_type: null,
      bytes: null,
      source_content: null,
      annotation: "",
      summary: `Captured ${kind} source`,
      relevance: "User-captured research Evidence",
      research_session_id: researchSessionId || null,
      research_question_id: null,
      capture_session_id: captureSessionId,
      classification: "internal" as const,
      metadata: { explicit_user_action: true },
    };
  }

  return <section className="capture-panel" aria-labelledby="capture-heading">
    {visibleIndicator && <div id="capture-status-banner" className={`capture-indicator ${uiState}`} role="status" aria-live="assertive">
      <span className="recording-dot" aria-hidden="true" />
      {uiState === "requesting" ? "Waiting for operating-system permission" : uiState === "preparing" ? "Permission granted — preparing recorder and waiting for the first media data. Selected sources are active." : uiState === "paused" ? "Capture paused — sensors may still be reserved" : uiState === "finalizing" ? "Finalizing capture safely" : "Screen or audio capture is active"}
    </div>}
    <div className="capture-heading-row"><div><p className="eyebrow">CP9 · Research capture</p><h2 id="capture-heading">Capture the work while it happens</h2><p>Record a bounded session, leave bookmarks, or attach source material as typed Evidence.</p></div><span className={`capture-state state-${uiState}`}>{uiState}</span></div>
    {!projectOpen && <p className="capture-gate">Open a Continuum project above to enable capture.</p>}
    {projectOpen && !researchEnabled && <p className="capture-gate">Research Space is disabled. Development work remains available; enable Research Space only when you want to capture Evidence.</p>}
    {error && <p className="error" role="alert">{error}</p>}
    {notice && <p className="notice" aria-live="polite">{notice}</p>}
    {runtimeHealth && <details className="capture-runtime"><summary>Recording diagnostics</summary><p>{runtimeHealth.summary} Device access is confirmed only after permission succeeds.</p></details>}

    <div className="capture-grid">
      <div className="capture-card">
        <h3>Live session</h3>
        <label className="research-session-select">Research session<select value={researchSessionId} disabled={!captureAvailable || visibleIndicator} onChange={(event) => setResearchSessionId(event.target.value)}><option value="">Unassigned Evidence</option>{researchSessions.map((item) => <option key={item.entity_id} value={item.entity_id}>{item.title}</option>)}</select></label>
        <div className="source-options">
          {(["screen", "system_audio", "microphone"] as CaptureSourceKind[]).map((kind) => <label key={kind}><input type="checkbox" checked={selection[kind]} disabled={!captureAvailable || visibleIndicator} onChange={(event) => setSource(kind, event.target.checked)} />{sourceLabel(kind)}</label>)}
        </div>
        {selection.system_audio && runtimeHealth?.platform === "windows" && <small>In the Windows sharing dialog, enable Share audio. If the selected screen or window provides no audio track, Continuum will not save a silent recording as though it contained system sound.</small>}
        {selection.system_audio && runtimeHealth?.native_system_audio_available && <small>System audio records the sound playing through your current output device. Microphone is selected separately.</small>}
        <label className="consent"><input type="checkbox" checked={consent} disabled={!captureAvailable || visibleIndicator} onChange={(event) => setConsent(event.target.checked)} />I confirm I have permission to capture this screen and audio.</label>
        <div className="capture-actions">
          {!["capturing", "paused", "finalizing"].includes(uiState) && <button className="primary" disabled={!canStart} onClick={startRecording}>Start capture</button>}
          {["requesting", "preparing"].includes(uiState) && !selection.screen && <button onClick={()=>permissionRequest.current?.abort()}>Cancel request</button>}
          {uiState === "capturing" && !nativeScreenActive && <button onClick={pauseRecording}>Pause</button>}
          {uiState === "paused" && !nativeScreenActive && <button onClick={resumeRecording}>Resume</button>}
          {["capturing", "paused"].includes(uiState) && <button className="danger" onClick={stopRecording}>Stop</button>}
          {nativeSavePending && <button className="primary" onClick={()=>void stopNativeScreenRecording()}>Retry save to Workspace</button>}
        </div>
        {nativeScreenActive && <div className="recording-description"><label>Recording title<input value={recordingTitle} maxLength={500} onChange={event=>setRecordingTitle(event.target.value)} placeholder="What does this recording show?" /></label><label>Description · optional<textarea value={recordingNote} maxLength={10000} onChange={event=>setRecordingNote(event.target.value)} placeholder="Experiment, observation, or result to remember" /></label><small>Screen{selection.system_audio ? " + System audio" : ""}{selection.microphone ? " + Microphone" : ""} · up to one minute. Stop & save adds the recording directly to Workspace.</small></div>}
        {["capturing", "paused"].includes(uiState) && !nativeScreenActive && <div className="marker-row"><input aria-label="Bookmark label" value={markerLabel} maxLength={500} onChange={(event) => setMarkerLabel(event.target.value)} /><button onClick={addMarker}>Add bookmark</button></div>}
        {session && <p className="session-meta">Session {session.id.slice(0, 8)} · {session.state} · {session.sources.map((item) => `${sourceLabel(item.source_kind)}: ${item.permission_status}`).join(" · ")}</p>}
      </div>

      <div className="capture-card">
        <h3>Quick Evidence</h3>
        <p>These actions never turn on a sensor in the background.</p>
        <div className="quick-actions"><button disabled={!captureAvailable || !consent || visibleIndicator} onClick={captureScreenshot}>Take screenshot</button><label className={`file-button ${!captureAvailable ? "disabled" : ""}`}>Import file<input type="file" disabled={!captureAvailable} onChange={captureFile} /></label></div>
        <details><summary>Add a web source or excerpt</summary><div className="web-source-form">
          <select aria-label="Web source kind" value={sourceKind} onChange={(event) => setSourceKind(event.target.value as typeof sourceKind)}><option value="web">Web research</option><option value="browser">Browser capture</option></select>
          <input aria-label="Source URL" placeholder="https://…" value={sourceUrl} onChange={(event) => setSourceUrl(event.target.value)} />
          <input aria-label="Source title" placeholder="Source title" value={sourceTitle} onChange={(event) => setSourceTitle(event.target.value)} />
          <textarea aria-label="Captured excerpt" placeholder="Paste the relevant excerpt or observation" value={sourceExcerpt} onChange={(event) => setSourceExcerpt(event.target.value)} />
          <button disabled={!captureAvailable} onClick={captureWebSource}>Save source as Evidence</button>
        </div></details>
      </div>
    </div>
    {previousRecordings.length>0&&!visibleIndicator&&<details className="capture-review"><summary>Recent saved recordings ({previousRecordings.length})</summary><p>Your recordings survive closing the app. These are the latest 25 capture sessions.</p>{previousRecordings.map(item=><div className="page-actions" key={item.id}><span>{new Date(item.created_at).toLocaleString()} · {item.sources.map(s=>sourceLabel(s.source_kind)).join(" + ")} · {item.state}</span><button onClick={()=>void openRecording(item)}>Review recording</button></div>)}</details>}
    {(capturedSegments.length > 0 || capturedMarkers.length > 0) && <div className="capture-review">
      <div><p className="eyebrow">Recording saved</p><h3>Add this recording to your workspace</h3></div>
      {session?.state==="completed"&&capturedSegments.length>0&&<div className="recording-description"><label>Recording title<input value={recordingTitle} maxLength={500} onChange={e=>setRecordingTitle(e.target.value)} placeholder="What does this recording show?" disabled={savedRecording||savingRecording}/></label><label>Description · optional<textarea value={recordingNote} maxLength={10000} onChange={e=>setRecordingNote(e.target.value)} placeholder="Experiment, observation, or result to remember" disabled={savedRecording||savingRecording}/></label><button disabled={!recordingTitle.trim()||savingRecording||savedRecording} onClick={()=>void saveRecording()}>{savedRecording?"Added to workspace":savingRecording?"Preparing your recording…":"Add complete recording to workspace"}</button><small>Workspace playback currently supports recordings up to 32 MiB. Original segments remain saved even if assembly fails.</small></div>}
      {preview && <div className="media-preview">{preview.mediaType.startsWith("video/") ? <video controls src={preview.url}>Video preview unavailable.</video> : preview.mediaType.startsWith("audio/") ? <audio controls src={preview.url}>Audio preview unavailable.</audio> : <p>Preview is not supported for {preview.mediaType}.</p>}</div>}
      <div className="review-list">
        <details><summary>Saved original segments ({capturedSegments.length})</summary><p>These are recovery fragments of one recording, not separate playable clips. Add the complete recording above for playback.</p>{capturedSegments.map((item) => <article key={item.id}><div><strong>Segment {item.sequence + 1}</strong><small>{formatOffset(item.start_offset_ms)}–{formatOffset(item.end_offset_ms)} · {formatBytes(item.byte_size)} · {item.state}</small></div>{capturedSegments.length===1&&<button onClick={() => previewSegment(item)}>Preview</button>}</article>)}</details>
        {capturedMarkers.map((item) => <article key={item.id}><div><strong>{item.label}</strong><small>{formatOffset(item.offset_ms)} · {item.note || "No note"}</small></div><button onClick={() => promoteMarker(item)}>Make Evidence</button></article>)}
      </div>
    </div>}
  </section>;
}

function sourceLabel(kind: CaptureSourceKind) {
  return kind === "screen" ? "Screen" : kind === "system_audio" ? "System audio" : "Microphone";
}

function errorMessage(cause: unknown) {
  return cause instanceof Error ? cause.message : String(cause);
}

function formatOffset(milliseconds: number) {
  const seconds = Math.floor(milliseconds / 1_000);
  return `${Math.floor(seconds / 60)}:${String(seconds % 60).padStart(2, "0")}`;
}

function formatBytes(bytes: number) {
  return bytes >= 1024 * 1024 ? `${(bytes / 1024 / 1024).toFixed(1)} MiB` : `${Math.max(1, Math.round(bytes / 1024))} KiB`;
}
