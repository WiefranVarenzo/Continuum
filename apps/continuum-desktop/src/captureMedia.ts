export function stopStream(stream: MediaStream) { stream.getTracks().forEach(track => track.stop()); }

/** PipeWire can hand WebKit DMA-BUF screen frames which preview correctly but
 * its direct MediaRecorder path never emits a chunk. Drawing the user-approved
 * preview to a canvas gives MediaRecorder a regular video track instead.
 * The preview is visible and every resource is stopped with the capture. */
export async function relayScreen(stream: MediaStream, signal: AbortSignal): Promise<{stream:MediaStream;release:()=>void}> {
  const video=document.createElement("video");
  video.muted=true;video.autoplay=true;video.playsInline=true;
  video.setAttribute("aria-label","Selected screen recording preview");
  video.style.cssText="position:fixed;right:16px;bottom:16px;width:160px;max-height:120px;z-index:1000;border:2px solid #69d7ac;border-radius:8px;background:#0d1914;pointer-events:none";
  const canvas=document.createElement("canvas");
  const context=canvas.getContext("2d",{alpha:false});
  if(!context || typeof canvas.captureStream!=="function")throw new Error("This desktop WebView cannot prepare a screen recording canvas.");
  let relay:MediaStream|null=null,timer:ReturnType<typeof setInterval>|null=null,closed=false;
  const release=()=>{if(closed)return;closed=true;if(timer)clearInterval(timer);relay?.getTracks().forEach(track=>track.stop());video.pause();video.srcObject=null;video.remove();};
  signal.addEventListener("abort",release,{once:true});
  try {
    video.srcObject=stream;document.body.appendChild(video);
    await new Promise<void>((resolve,reject)=>{
      const finish=(error?:Error)=>{clearTimeout(timeout);signal.removeEventListener("abort",cancel);if(error)reject(error);else resolve();};
      const cancel=()=>finish(new DOMException("Capture cancelled.","AbortError"));
      const timeout=setTimeout(()=>finish(new Error("The selected screen preview did not start within 8 seconds.")),8000);
      signal.addEventListener("abort",cancel,{once:true});
      if(signal.aborted){cancel();return;}
      void video.play().then(()=>finish(),cause=>finish(cause instanceof Error?cause:new Error(String(cause))));
    });
    if(signal.aborted)throw new DOMException("Capture cancelled.","AbortError");
    if(!video.videoWidth || !video.videoHeight)throw new Error("The selected screen has no video frames yet. Choose another window and retry.");
    canvas.width=video.videoWidth;canvas.height=video.videoHeight;
    const draw=()=>{if(!closed&&video.readyState>=HTMLMediaElement.HAVE_CURRENT_DATA)try{context.drawImage(video,0,0,canvas.width,canvas.height);}catch{ /* The recorder watchdog reports a bounded failure if frames never encode. */ }};
    draw();relay=canvas.captureStream(30);timer=setInterval(draw,1000/30);
    return {stream:relay,release:()=>{signal.removeEventListener("abort",release);release();}};
  } catch(cause){signal.removeEventListener("abort",release);release();throw cause;}
}

/** A recorder may start successfully but silently produce no media (missing encoder).
 * Do not claim Recording until a real encoded chunk arrives. Sensors stay visible
 * as Preparing and cancellable during this bounded startup check. */
export function startRecorder(recorder: MediaRecorder, signal: AbortSignal): Promise<void> {
  return new Promise((resolve, reject) => {
    const finish = (error?: Error) => {
      clearTimeout(timer);
      recorder.removeEventListener("dataavailable", data);
      recorder.removeEventListener("error", failed);
      recorder.removeEventListener("stop", stopped);
      signal.removeEventListener("abort", cancel);
      if (error) reject(error); else resolve();
    };
    const data = (event: BlobEvent) => { if (event.data.size > 0) finish(); };
    const failed = (event: Event) => finish(new Error((event as Event & {error?: Error}).error?.message || "The recording encoder failed to start."));
    const stopped = () => finish(new Error("The recording stopped before producing media data."));
    const cancel = () => finish(new DOMException("Capture cancelled.", "AbortError"));
    const timer = setTimeout(() => finish(new Error("Screen/audio access succeeded, but the recorder produced no data within 15 seconds. The recording runtime or encoder could not start; this is not a permission denial.")), 15_000);
    recorder.addEventListener("dataavailable", data);
    recorder.addEventListener("error", failed);
    recorder.addEventListener("stop", stopped);
    signal.addEventListener("abort", cancel, {once:true});
    if (signal.aborted) { cancel(); return; }
    try { recorder.start(5_000); } catch (error) { finish(error instanceof Error ? error : new Error(String(error))); }
  });
}

/** Called synchronously from the click handler. Late grants are always released. */
export function requestMediaPermission(request: Promise<MediaStream>, label: string, signal?: AbortSignal): Promise<MediaStream> {
  return new Promise((resolve, reject) => {
    let settled = false;
    const finish = (error?: Error, stream?: MediaStream) => {
      if (settled) { if (stream) stopStream(stream); return; }
      settled = true; clearTimeout(timer); signal?.removeEventListener("abort", cancel);
      if (error) reject(error); else resolve(stream!);
    };
    const cancel = () => finish(new DOMException("Capture cancelled. No recording was saved.", "AbortError"));
    const timer = setTimeout(() => finish(new Error(`The ${label} permission request did not finish within 90 seconds. Check the Continuum permission dialog and your desktop's screen chooser, then retry.`)), 90_000);
    signal?.addEventListener("abort", cancel, {once:true});
    request.then(stream => finish(undefined, stream), error => finish(error instanceof Error || error instanceof DOMException ? error : new Error(String(error))));
    if (signal?.aborted) cancel();
  });
}

export function captureError(cause: unknown, platform = "linux"): string {
  const name = cause instanceof Error || cause instanceof DOMException ? cause.name : "";
  const detail = cause instanceof Error || cause instanceof DOMException ? cause.message : String(cause);
  if (name === "NotAllowedError") return "Capture permission was declined or blocked. Start again, allow the Continuum permission dialog, then select a screen/window in the desktop chooser. No recording has started.";
  if (name === "InvalidStateError") return "The screen chooser must open directly from your click. Return to this window and press Start capture again.";
  if (name === "NotFoundError") return "No matching recording device was found. Check the selected source and your desktop Sound settings.";
  if (name === "NotReadableError") return platform === "windows"
    ? "The selected device could not start. Check Windows Privacy & security permissions, the sharing dialog, and whether another app is using the device."
    : "The selected device could not start. Check whether another app is using it and whether PipeWire is running, then retry.";
  if (name === "NotSupportedError") return platform === "windows"
    ? `WebView2 could not create a recording encoder. Update the Microsoft Edge WebView2 Runtime and retry. Details: ${detail}`
    : `The recording engine could not create an encoder. This is separate from screen/microphone permission. Use the updated Continuum package with its recording plugins. Details: ${detail}`;
  return detail;
}

/** MediaRecorder accepts one mixed audio track reliably across WebViews. */
export async function recordingStream(tracks: MediaStreamTrack[], preparedContext?:AudioContext): Promise<{stream:MediaStream;release:()=>void}> {
  const audio = tracks.filter(track => track.kind === "audio");
  if (audio.length <= 1) return {stream:new MediaStream(tracks), release:()=>{}};
  const context = preparedContext ?? new AudioContext();
  try {
    const destination = context.createMediaStreamDestination();
    for (const track of audio) context.createMediaStreamSource(new MediaStream([track])).connect(destination);
    await context.resume();
    return {stream:new MediaStream([...tracks.filter(track=>track.kind==="video"),...destination.stream.getAudioTracks()]), release:()=>{stopStream(destination.stream);if(!preparedContext)void context.close();}};
  } catch (error) { if(!preparedContext)void context.close(); throw error; }
}
