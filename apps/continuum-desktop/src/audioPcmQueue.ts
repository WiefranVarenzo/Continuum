/** Bounded stereo queue for the audio render thread. Input is 48 kHz PCM16. */
export class AudioPcmQueue {
  private readonly samples: Float32Array;
  private readFrame = 0;
  private writeFrame = 0;
  private frames = 0;
  private started = false;

  constructor(readonly capacityFrames = 12_000, readonly prefillFrames = 4_800) {
    this.samples = new Float32Array(capacityFrames * 2);
  }

  get queuedFrames() { return this.frames; }

  push(pcm: Int16Array) {
    const incoming = Math.floor(pcm.length / 2);
    const retained = Math.min(incoming, this.capacityFrames);
    const discard = Math.max(0, this.frames + retained - this.capacityFrames);
    this.readFrame = (this.readFrame + discard) % this.capacityFrames;
    this.frames -= discard;
    // After a delayed IPC burst retain current sound, rather than building an
    // ever-growing delay or terminating the entire screen recording.
    const offset = incoming - retained;
    for (let frame = offset; frame < incoming; frame++) {
      this.samples[this.writeFrame * 2] = pcm[frame * 2] / 32768;
      this.samples[this.writeFrame * 2 + 1] = pcm[frame * 2 + 1] / 32768;
      this.writeFrame = (this.writeFrame + 1) % this.capacityFrames;
    }
    this.frames += retained;
  }

  render(left: Float32Array, right: Float32Array) {
    left.fill(0); right.fill(0);
    if (!this.started && this.frames >= this.prefillFrames) this.started = true;
    if (!this.started) return;
    const available = Math.min(left.length, right.length, this.frames);
    for (let frame = 0; frame < available; frame++) {
      left[frame] = this.samples[this.readFrame * 2];
      right[frame] = this.samples[this.readFrame * 2 + 1];
      this.readFrame = (this.readFrame + 1) % this.capacityFrames;
    }
    this.frames -= available;
    // Silence preserves the recording's clock across a UI/IPC underrun.
    // Rebuffer briefly when packets return; ordinary jitter is not a failure.
    if (available < left.length) this.started = false;
  }
}
