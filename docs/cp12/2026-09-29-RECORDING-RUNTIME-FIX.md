# Recording runtime repair — 29 September 2026

> Follow-up: actual screen capture still fails in this release. See
> `2026-09-29-SCREEN-SOURCE-FOLLOWUP.md`. The earlier GL warning is not optional
> for the user's DMA-BUF screen source. Synthetic CPU video success does not
> certify real screen recording.

## Confirmed failure

User screenshots show Screen granted/ready, followed by `The MediaRecorder is unsupported on this platform`. This is not a portal permission denial. WebKit's [MediaRecorderPrivateGStreamer](https://github.com/WebKit/WebKit/blob/main/Source/WebCore/platform/mediarecorder/MediaRecorderPrivateGStreamer.cpp) creates `uritranscodebin`; failure to construct the pipeline returns the reported unsupported error.

The installed host has this factory. Running `gst-inspect-1.0 uritranscodebin` with the prior AppDir libraries and packaged plugin path returned `No such element or plugin`. AppImage relocates GStreamer system paths, so the previously packaged codecs/appsrc/appsink alone were insufficient.

## Changes

- `packaging/prepare-gstreamer-plugins.sh`: include transcode, encoding, playback, coreelements, typefindfunctions, videorate, audiorate, autoconvert, videofilter, volume, audiofx, deinterlace, debugutilsbad and opusparse alongside existing capture/codec plugins.
- Native `recording_runtime.rs`: call stable GStreamer C ABI through Linux runtime SONAME (no additional crate or development symlink dependency). Initialize runtime and construct required elements in NULL state. No sensor is accessed. Health diagnostics distinguish recording-component availability from portal/device permission and actual output.
- `CapturePanel`: explicit Preparing state after grants, fail-fast unsupported format/runtime, cancellable bounded recorder startup and cleanup; busy navigation protection is preserved. Workspace top bar receives the real phase rather than labelling all busy states Recording.
- `startRecorder`: require nonempty data within 15 seconds; handle cancellation, stop, synchronous errors and asynchronous errors. Never confuse encoder failures with permission errors.
- Explicit Opus is required for audio+video so a fallback cannot silently select a different audio codec while metadata still says Opus.
- `qa/recording_probe.py` and `packaging/check-recording-runtime.sh`: reproducible sensor-free runtime integration test. Uses actual built AppDir/WebKit libraries and bundled plugins with `GST_PLUGIN_SYSTEM_PATH_1_0` empty; records synthetic canvas video and oscillator audio at the production five-second chunk interval, validates header and actual playback advancement. Run from repository root with `bash packaging/check-recording-runtime.sh apps/continuum-desktop/src-tauri/target/release/bundle/appimage/Continuum.AppDir`.

## Evidence

- Focused UI tests: 16 passed across CapturePanel, captureMedia, captureRuntime.
- `cargo test --offline recording_runtime`: 1 passed. No live AI provider call or personal recording.
- TypeScript/Vite production build and Tauri AppImage build passed. Existing bundle-size/dynamic-import warnings remain unrelated.
- Packaged runtime probe exited 0: video 133099 bytes / 2 chunks / EBML header / 320×240 / playback 0.7007s; audio 152455 bytes / 2 chunks / EBML header / playback 0.6814s. Generated media retained in `/tmp/continuum-media-proof-pk9d7cmz` (temporary, not user project data).
- WebKit still warns about absent optional WebVTT encoder and GL video sink; software playback passed. No claim of hardware acceleration or subtitle support.

## Remaining limitation — do not mark combined recording complete

The optional `--include-combined` regression demonstrates another issue: combining canvas video and Web Audio yields mismatched PTS. `ffprobe` showed video start around 25046.309s and audio around 0.023s. Both tracks contain data but combined playback did not advance. Native system audio is fed through Web Audio, so Screen + native System audio is explicitly rejected before any sensor opens. This prevents knowingly producing an unusable combined recording. It does not implement synchronized native A/V.

Follow-up needs a properly synchronized native pipeline or a bounded, provenance-preserving remux/clock-normalization step with tests for real screen/native system audio/microphone. Do not compensate by arbitrary guessed offsets. Screen-only and audio-only runtime fixes are verified, while actual portal selection, real-device playback and Screen + direct microphone remain user-interactive acceptance checks.

## Release

`releases/0.12.0-recording-fix-2026-09-29/Continuum_0.12.0_amd64.AppImage`

121424376 bytes; SHA256 `e3f2b25044b4229d5995a4e6c71573a61a3c166c938d9c9c5b09e39294587b36`.

Previous releases and databases were not replaced. No claim that all capture combinations or the whole application are now certified.

The copied release checksum verified successfully. Isolated startup stayed alive for the full 10-second smoke check with no output/errors; timeout exit 124 was expected. No personal project was opened during this check.
