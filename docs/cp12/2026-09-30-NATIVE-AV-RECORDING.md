# Native screen + audio follow-up — 30 September 2026

## Change

The Linux screen recorder uses the portal screen and, when selected, PulseAudio-compatible system monitor and/or microphone. Video is VP8 and audio is Opus in WebM. A single selected audio source bypasses the live mixer. When both audio sources are selected, `parec` captures each into a private, bounded PCM file while the screen is recorded separately; a finite offline GStreamer pipeline mixes the two audio files and muxes them with the video after Stop. The WebView mixed-clock recorder remains unused whenever Screen is selected. Audio-only capture keeps its existing path.

The native command requires the user's capture consent. Merely opening the project or rendering the capture page does not start a sensor. Closing the recording page finalizes the active capture as before; the portal session, child audio recorders, and temporary files are released on failure or stop. The AppImage bundles `libgstaudiomixer.so` and `libgstrawparse.so` as well as the previously bundled PulseAudio and Opus plugins. Three-source recording requires `/usr/bin/parec` on the Linux host.

## Verification completed without user media

- Native recorder unit tests passed, including single-audio abrupt-stop/remux and two-audio finite offline mixing without opening sensors.
- CapturePanel tests passed for Screen + system audio, Screen + microphone, and all three. The tests assert that no WebView display/microphone request is made for these combinations.
- TypeScript/Vite and Rust builds passed; an AppImage was built at `apps/continuum-desktop/src-tauri/target/release/bundle/appimage/Continuum_0.12.0_amd64.AppImage`.
- Sensor-free GStreamer tests produced decodable VP8/Opus WebM for the one-audio abrupt-stop path and the two-audio offline-mix path. The rebuilt AppDir contains the bundled audiomixer and raw-audio parser plugins, and the packaged sensor-free media runtime probe passed.

## Acceptance still required

On the user's Fedora session, record a non-sensitive window for 5–10 seconds in each mode: Screen + microphone, Screen + system audio while a sound is playing, and all three. Open the saved Workspace evidence, verify moving video and audible selected sources, and check that stopping does not crash. Synthetic sources and plugin availability do not prove real-device permission, routing, audio level, or playback on every machine.

The first real-device build force-quit while Stop sent EOS to the live audio pipeline. A subsequent streamable-mux build avoided the force quit but did not save a recording; Stop reported an empty or missing output and the UI incorrectly offered a retry after the backend had already released it. The retry state is now limited to persistence failures, and single-audio live mixing has been removed. Wiefran verified the **development build** with Screen + Microphone and Screen + System audio: saved Workspace video moves and the selected sound is audible. The first three-source build still failed, so the two live audio sources were moved into independent PCM captures followed by offline mixing. Wiefran subsequently reported the Workspace result was correct. A packaged AppImage must still be checked separately from the development build.

The earlier release under `releases/0.12.0/` was not replaced. The generated AppImage at the bundle path above was rebuilt after the live Workspace test and passed its sensor-free packaged media probe. Its Screen + audio modes have not yet been repeated inside the packaged AppImage itself; the verified user-device run was the development build.
