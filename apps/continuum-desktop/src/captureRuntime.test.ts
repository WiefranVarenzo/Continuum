import { describe, expect, it } from "vitest";
import {
  CAPTURE_BUFFER_POLICY,
  chooseRecorderMimeType,
  evaluateCapturePressure,
  observedTrackPermission,
  requestedCaptureSources,
} from "./captureRuntime";

describe("CP9 capture runtime safety", () => {
  it("maps only explicitly selected sensors", () => {
    expect(requestedCaptureSources({ screen: true, system_audio: false, microphone: true }))
      .toEqual(["screen", "microphone"]);
  });

  it("chooses a supported bounded recording format without inventing support", () => {
    expect(chooseRecorderMimeType(true, (value) => value === "video/webm"))
      .toBe("video/webm");
    expect(chooseRecorderMimeType(false, () => false)).toBe("");
    expect(chooseRecorderMimeType(true, value => value === "video/webm", true)).toBe("");
    expect(chooseRecorderMimeType(true, () => true, true)).toBe("video/webm;codecs=vp8,opus");
  });

  it("flushes early and pauses before the bounded queue can grow without limit", () => {
    expect(evaluateCapturePressure(CAPTURE_BUFFER_POLICY, 1, 0)).toBe("continue");
    expect(evaluateCapturePressure(CAPTURE_BUFFER_POLICY, 13 * 1024 * 1024, 1)).toBe("flush_segment");
    expect(evaluateCapturePressure(CAPTURE_BUFFER_POLICY, 16 * 1024 * 1024, 1)).toBe("pause");
    expect(evaluateCapturePressure(CAPTURE_BUFFER_POLICY, 1, 4)).toBe("pause");
  });

  it("never reports a source grant when the operating system returned no matching track", () => {
    const noTracks = { getVideoTracks: () => [], getAudioTracks: () => [] };
    const screenTrack = { getVideoTracks: () => [{} as MediaStreamTrack], getAudioTracks: () => [] };
    const audioTrack = { getVideoTracks: () => [], getAudioTracks: () => [{} as MediaStreamTrack] };
    expect(observedTrackPermission(noTracks, "screen")).toBe("unavailable");
    expect(observedTrackPermission(noTracks, "microphone")).toBe("unavailable");
    expect(observedTrackPermission(screenTrack, "screen")).toBe("granted");
    expect(observedTrackPermission(audioTrack, "system_audio")).toBe("granted");
  });
});
