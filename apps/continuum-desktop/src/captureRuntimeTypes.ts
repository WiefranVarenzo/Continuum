import type { CaptureBufferPolicy, CaptureCapability, CaptureSourceKind } from "./captureContracts";

export type { CaptureBufferPolicy, CaptureCapability, CaptureSourceKind };
export type CaptureBackpressureAction = "continue" | "flush_segment" | "reduce_quality" | "pause";
