import { Channel, invoke } from "@tauri-apps/api/core";
import processorUrl from "./systemAudioProcessor?worker&url";
interface AudioPacket { id: string; sequence: number; samples: number[]; error: string | null }

export async function desktopAudio(projectId: string, signal: AbortSignal, onError: (error: Error) => void, preparedContext?: AudioContext) {
  const context = preparedContext ?? new AudioContext({ sampleRate: 48000 });
  const destination = context.createMediaStreamDestination();
  const scheduled = new Set<AudioBufferSourceNode>();
  let renderer: AudioWorkletNode | undefined;
  let id = "", closed = false, received = false, nextTime = 0;
  let lastPacket = Date.now();
  let readyResolve!: () => void, readyReject!: (error: Error) => void;
  const ready = new Promise<void>((resolve, reject) => { readyResolve = resolve; readyReject = reject; });
  void ready.catch(() => {});
  const release = () => {
    if (closed) return;
    closed = true;
    clearTimeout(timer); clearInterval(watchdog);
    signal.removeEventListener("abort", abort);
    if (renderer) { renderer.port.postMessage({ stop: true }); renderer.port.close(); renderer.disconnect(); }
    scheduled.forEach(source => { try { source.stop(); } catch { /* already ended */ } source.disconnect(); });
    scheduled.clear();
    destination.stream.getTracks().forEach(track => track.stop());
    if (!preparedContext) void context.close();
    if (id) void invoke("stop_system_audio", { id }).catch(() => {});
  };
  const fail = (error: Error) => { if (closed) return; readyReject(error); release(); if (received) onError(error); };
  const abort = () => fail(new DOMException("Capture cancelled.", "AbortError"));
  const timer = setTimeout(() => fail(new Error("Desktop audio did not start. Check your default output device and restart capture.")), 12_000);
  const watchdog = setInterval(() => {
    if (received && Date.now() - lastPacket > 3000) fail(new Error("Desktop audio stopped responding. Recording stopped; saved segments remain available."));
  }, 1000);
  signal.addEventListener("abort", abort, { once: true });

  const onData = new Channel<AudioPacket>();
  onData.onmessage = packet => {
    id = packet.id;
    if (closed) { void invoke("stop_system_audio", { id }).catch(() => {}); return; }
    if (packet.error) { fail(new Error(packet.error)); return; }
    try {
      if (packet.samples.length !== 4800) throw new Error("Invalid desktop audio packet; recording stopped safely.");
      lastPacket = Date.now();
      if (renderer) {
        const pcm = Int16Array.from(packet.samples);
        renderer.port.postMessage(pcm, [pcm.buffer]);
      } else {
        // Compatibility for older WebViews without AudioWorklet. Keep at most
        // 250 ms scheduled, and re-anchor after an underrun instead of stopping.
        const now = context.currentTime;
        if (nextTime < now + 0.01) nextTime = now + 0.04;
        if (nextTime <= now + 0.25) {
          const buffer = context.createBuffer(2, 2400, 48000);
          for (let channel = 0; channel < 2; channel++) {
            const data = buffer.getChannelData(channel);
            for (let i = 0; i < 2400; i++) data[i] = packet.samples[i * 2 + channel] / 32768;
          }
          const source = context.createBufferSource();
          source.buffer = buffer; source.connect(destination); scheduled.add(source);
          source.onended = () => { source.disconnect(); scheduled.delete(source); };
          source.start(nextTime); nextTime += 0.05;
        }
      }
      received = true; clearTimeout(timer); readyResolve();
      void invoke("acknowledge_system_audio", { id, sequence: packet.sequence }).catch(error => fail(new Error(String(error))));
    } catch (error) { fail(error instanceof Error ? error : new Error(String(error))); }
  };
  try {
    if (signal.aborted) throw new DOMException("Capture cancelled.", "AbortError");
    await context.resume();
    if (context.audioWorklet && typeof AudioWorkletNode !== "undefined") {
      await context.audioWorklet.addModule(processorUrl);
      if (closed) throw new DOMException("Capture cancelled.", "AbortError");
      renderer = new AudioWorkletNode(context, "continuum-system-audio", { numberOfInputs: 0, numberOfOutputs: 1, outputChannelCount: [2] });
      renderer.onprocessorerror = () => fail(new Error("The system audio renderer stopped. Restart capture; saved segments remain available."));
      renderer.connect(destination);
    }
    id = await invoke<string>("start_system_audio", { projectId, consent: true, onData });
    if (closed) { await invoke("stop_system_audio", { id }); throw new DOMException("Capture cancelled.", "AbortError"); }
    await ready;
    return { stream: destination.stream, release };
  } catch (error) { release(); throw error; }
}
