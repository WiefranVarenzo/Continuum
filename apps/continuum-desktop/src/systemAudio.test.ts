import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import { desktopAudio } from "./systemAudio";

vi.mock("./systemAudioProcessor?worker&url", () => ({ default: "/audio-processor.js" }));
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn(), Channel: class { onmessage: (packet: unknown) => void = () => {}; } }));
let deliver!: (packet: unknown) => void;
const packet = (sequence: number) => ({ id: "native-audio", sequence, samples: new Array(4800).fill(8192), error: null });
const track = { stop: vi.fn() };
let context: FakeContext, worklet: FakeWorklet;
class FakeContext {
  currentTime = 0;
  resume = vi.fn(async () => {});
  close = vi.fn(async () => {});
  audioWorklet: { addModule: ReturnType<typeof vi.fn> } | undefined = { addModule: vi.fn(async () => {}) };
  createMediaStreamDestination() { return { stream: { getTracks: () => [track] } }; }
  createBuffer() { return { getChannelData: () => new Float32Array(2400) }; }
  createBufferSource() { return { connect: vi.fn(), disconnect: vi.fn(), start: vi.fn(), stop: vi.fn(), onended: undefined }; }
}
class FakeWorklet {
  port = { postMessage: vi.fn(), close: vi.fn() };
  connect = vi.fn(); disconnect = vi.fn(); onprocessorerror?: () => void;
  constructor() { worklet = this; }
}

beforeEach(() => {
  vi.useFakeTimers(); track.stop.mockClear(); context = new FakeContext();
  vi.stubGlobal("AudioContext", class { constructor() { return context; } });
  vi.stubGlobal("AudioWorkletNode", FakeWorklet);
  vi.mocked(invoke).mockReset().mockImplementation(async (command, input) => {
    if (command === "start_system_audio") {
      deliver = (input as { onData: { onmessage: typeof deliver } }).onData.onmessage;
      Promise.resolve().then(() => deliver(packet(1)));
      return "native-audio";
    }
    return undefined;
  });
});
afterEach(() => { vi.useRealTimers(); vi.unstubAllGlobals(); });

describe("desktop audio relay lifecycle", () => {
  it("uses the render thread and acknowledges a delayed burst without a synchronization error", async () => {
    const onError = vi.fn();
    const audio = await desktopAudio("project", new AbortController().signal, onError);
    context.currentTime = 2;
    for (let sequence = 2; sequence <= 50; sequence++) deliver(packet(sequence));
    expect(worklet.port.postMessage).toHaveBeenCalledTimes(50);
    expect(vi.mocked(invoke).mock.calls.filter(call => call[0] === "acknowledge_system_audio")).toHaveLength(50);
    expect(onError).not.toHaveBeenCalled();
    audio.release(); audio.release();
    expect(worklet.disconnect).toHaveBeenCalledTimes(1); expect(track.stop).toHaveBeenCalledTimes(1);
    expect(context.close).toHaveBeenCalledTimes(1); expect(vi.getTimerCount()).toBe(0);
  });
  it("recovers a compatibility renderer after a scheduling gap instead of stopping capture", async () => {
    context.audioWorklet = undefined;
    const onError = vi.fn();
    const audio = await desktopAudio("project", new AbortController().signal, onError);
    context.currentTime = 1;
    for (let sequence = 2; sequence <= 30; sequence++) deliver(packet(sequence));
    expect(onError).not.toHaveBeenCalled(); audio.release();
  });
  it("still stops on a genuinely stalled native source and preserves caller-owned context", async () => {
    const onError = vi.fn();
    await desktopAudio("project", new AbortController().signal, onError, context as unknown as AudioContext);
    await vi.advanceTimersByTimeAsync(4100);
    expect(onError).toHaveBeenCalledWith(expect.objectContaining({ message: expect.stringContaining("stopped responding") }));
    expect(context.close).not.toHaveBeenCalled(); expect(track.stop).toHaveBeenCalledTimes(1);
  });
  it("rejects malformed packets and shuts down the native capture", async () => {
    const onError = vi.fn();
    await desktopAudio("project", new AbortController().signal, onError);
    deliver({ ...packet(2), samples: [0] });
    expect(onError).toHaveBeenCalledWith(expect.objectContaining({ message: expect.stringContaining("Invalid desktop audio packet") }));
    expect(vi.mocked(invoke).mock.calls.some(call => call[0] === "stop_system_audio")).toBe(true);
  });
});
