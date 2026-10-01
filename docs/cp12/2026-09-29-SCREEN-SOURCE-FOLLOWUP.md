# Screen source follow-up — 29 September 2026

## Status: not fixed / no replacement release yet

The user confirmed both screen recording and Take screenshot fail. The release
`0.12.0-recording-fix-2026-09-29` is still the last release; do not claim it
supports real screen capture based on its passing synthetic video tests.

## User-authorized screen diagnostics

Temporary harness: `/tmp/continuum-screen-source-probe.py`. Gtk/WebKit 4.1,
AppDir libraries, GDK_BACKEND=x11. User explicitly presses Check shared screen,
approves app permission and chooses a non-sensitive source. Source stops after
20 seconds; no images or media are saved to disk. Only counters are printed.

1. Original harness permission callback incorrectly called a GI method missing
   from this typelib. Corrected it through the public WebKit C ABI. The first
   diagnostic NotAllowedError was a harness defect, not proof of user denial.
2. Original bundle: PipeWire negotiated 1920x1200
   `video/x-raw(memory:DMABuf), format=DMA_DRM, drm-format=AR24, framerate=0/1`.
   Granted track stayed live, preview failed as unsupported, zero frames/data.
   WebKit warned about missing GL video sink dependencies.
3. Added host libgstopengl.so through the existing packaging script. Same
   AppDir libraries and user selection now show a moving preview: 322 frames
   by 18 seconds, 1920x1200, readyState=3. User explicitly confirmed preview.
   Direct MediaRecorder still returned zero chunks/bytes.
4. A comparison of direct recorder and video-to-2D-canvas relay logged
   `videoconvertscale` refusing DMA_DRM and WebKit
   `video-frame-converter-dmabuf` / glupload returning not-negotiated. Output
   was very verbose; final relay counters were not recovered. Do not assert
   the relay works. Even nonzero bytes would require checking image content
   and playback, not accepting a blank canvas video.

## Retained changes

- Package `libgstopengl.so` (required for this actual source, not optional).
- Native component checks now instantiate glupload, glcolorconvert,
  gldownload and glsinkbin without starting sensors.
- Native runtime test passed. Production frontend build passed before
  removing an unproven experimental canvas relay. That relay was removed
  from CapturePanel and its new standalone files deleted; do not ship the
  experiment as a recording fix. Existing user work is preserved.
- No new AppImage built/copied. Frontend dist was rebuilt after removing the
  experiment, restoring the existing recording path.

## Next targeted investigation

- Ask for the comparison test's relayChunks/relayBytes output if necessary.
- Distinguish GL context/import failure from WebKit's frame conversion caps
  bug. Consider a diagnostic using EGL / native Wayland, but do not force
  graphics or security settings into production without proof.
- Another option is a properly scoped native portal/PipeWire capture path
  with supported raw-frame negotiation; do not bypass OS authorization.
- Keep debug categories narrow or filter counters before returning logs;
  GST_DEBUG=2 flooded output with per-frame conversion errors.
- Revalidate actual screenshots, recording bytes, playable content and stop/
  cancel cleanup. Prior screen + native system audio mixed-clock guard stays.

## Primary code references used

- WebKit webkitgtk-2.52.5: VideoFrameGStreamer.cpp (infoFromCaps,
  convertSampleToImage), GStreamerVideoFrameConverter.cpp (DMABufMemory
  glupload/conversion), GStreamerVideoCapturer.cpp (display conversion bypass).
- GStreamer documentation: DMA buffers and GstGLDisplay. EGL is relevant to
  DMA-BUF import; no environment-variable workaround has been validated yet.

No project database, personal evidence, or previous release was modified.
