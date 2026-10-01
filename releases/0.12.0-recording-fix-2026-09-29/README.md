# Continuum recording runtime fix — 29 September 2026

Open `Continuum_0.12.0_amd64.AppImage` from this folder, not the September 28 preview.

## Fixed

- WebKit recording plugins missing from the AppImage: the transcode/encoding chain, typefinding, audio/video rate conversion and playback helpers are now included.
- Recording diagnostics construct the real encoder/runtime elements without opening sensors.
- Permission, preparing, recording and saving are distinct UI states. Recording appears only after nonempty encoded media arrives; a silent encoder fails within 15 seconds and releases its sources.
- Unsupported recording formats fail before opening sensors instead of silently defaulting to a different container.

## Verified

- 16 focused UI/recording tests passed; native runtime-element test passed.
- Production build and AppImage packaging passed.
- Actual bundled WebKit, with system plugin search disabled: a generated 6.2-second video and a separate generated 6.2-second audio recording both produced two chunks, a valid WebM header, and successful playback. Video: 133,099 bytes and 320×240 decoded frame. Audio: 152,455 bytes. Playback time advanced for both.
- These tests use only synthetic canvas/oscillator media; they do not grant permission to record a real screen or microphone.

## Important limitation

Screen + native System audio is temporarily rejected **before opening sensors** on Linux. The WebKit mixed-clock test creates video timestamps near system uptime and audio timestamps near zero, yielding an unplayable combined result. This is not fixed by the plugin repair. Record screen separately, or deselect Screen for audio-only capture. Screen + direct microphone has not been interactively certified.

Real-device acceptance still requires the user to choose a screen in the desktop permission dialog: record Screen only for 10 seconds, stop, name it, add it to the workspace, then play it. Native GUI interaction was not automated or claimed verified.

Prior releases and project data are unchanged.

SHA256: `e3f2b25044b4229d5995a4e6c71573a61a3c166c938d9c9c5b09e39294587b36`
Size: 121,424,376 bytes.
