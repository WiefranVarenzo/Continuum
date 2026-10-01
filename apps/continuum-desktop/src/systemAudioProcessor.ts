import { AudioPcmQueue } from "./audioPcmQueue";

declare class AudioWorkletProcessor { readonly port: MessagePort; }
declare function registerProcessor(name: string, processor: typeof AudioWorkletProcessor): void;

class ContinuumSystemAudio extends AudioWorkletProcessor {
  private readonly queue = new AudioPcmQueue();
  private stopped = false;
  constructor() {
    super();
    this.port.onmessage = event => {
      if (event.data?.stop) { this.stopped = true; return; }
      if (event.data instanceof Int16Array) this.queue.push(event.data);
    };
  }
  process(_inputs: Float32Array[][], outputs: Float32Array[][]) {
    if (this.stopped) return false;
    const [left, right] = outputs[0] ?? [];
    if (left && right) this.queue.render(left, right);
    return true;
  }
}

registerProcessor("continuum-system-audio", ContinuumSystemAudio);
