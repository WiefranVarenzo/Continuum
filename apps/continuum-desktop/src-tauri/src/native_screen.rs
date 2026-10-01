//! Linux screen recording through the user-facing XDG ScreenCast portal.
//! WebKit can display DMA-BUF frames yet return black pixels to canvas; keep
//! those frames in PipeWire/GStreamer instead of copying them through the WebView.
//! A single audio source can share the GStreamer pipeline with the screen.
//! Two live Pulse sources can deadlock that pipeline on Stop, so the two-source
//! case records bounded raw PCM separately and mixes it only after capture.
use gio::prelude::*;
use gtk::glib::{self, ToVariant};
use std::cell::RefCell;
use std::collections::HashMap;
use std::ffi::{c_char, c_void, CString};
use std::fs::{self, OpenOptions};
use std::os::fd::{FromRawFd, OwnedFd};
use std::os::unix::fs::OpenOptionsExt;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const PORTAL: &str = "org.freedesktop.portal.Desktop";
const PORTAL_PATH: &str = "/org/freedesktop/portal/desktop";
const SCREENCAST: &str = "org.freedesktop.portal.ScreenCast";
const MAX_BYTES: u64 = 32 * 1024 * 1024;
const MAX_FRAMES: u32 = 1800; // 60 seconds at the negotiated 30 fps.
static TOKEN: AtomicU64 = AtomicU64::new(1);

pub fn trace_capture(stage: &str) {
    if std::env::var_os("CONTINUUM_CAPTURE_DIAGNOSTICS").is_some() {
        eprintln!("[continuum-capture] {stage}");
    }
}

#[link(name = "libgstreamer-1.0.so.0", kind = "dylib", modifiers = "+verbatim")]
unsafe extern "C" {
    fn gst_init_check(argc: *mut i32, argv: *mut *mut *mut c_char, error: *mut *mut c_void) -> i32;
    fn gst_parse_launch(description: *const c_char, error: *mut *mut glib::ffi::GError) -> *mut c_void;
    fn gst_element_set_state(element: *mut c_void, state: i32) -> i32;
    fn gst_element_get_state(element: *mut c_void, state: *mut i32, pending: *mut i32, timeout: u64) -> i32;
    fn gst_element_get_bus(element: *mut c_void) -> *mut c_void;
    fn gst_element_send_event(element: *mut c_void, event: *mut c_void) -> i32;
    fn gst_event_new_eos() -> *mut c_void;
    fn gst_bus_timed_pop_filtered(bus: *mut c_void, timeout: u64, types: u32) -> *mut c_void;
    fn gst_mini_object_unref(object: *mut c_void);
    fn gst_object_unref(object: *mut c_void);
}

pub struct RecordedScreen {
    pub bytes: Vec<u8>,
    pub duration_ms: u64,
    pub system_audio: bool,
    pub microphone: bool,
}

#[derive(Default)]
pub struct NativeScreenState {
    current: Mutex<Option<NativeScreen>>,
    ready: Mutex<Option<RecordedScreen>>,
}

struct NativeScreen {
    pipeline: *mut c_void,
    _portal: PortalLease,
    output: PathBuf,
    raw_audio: Vec<RawAudioCapture>,
    started: Instant,
    system_audio: bool,
    microphone: bool,
}

struct RawAudioCapture {
    path: PathBuf,
    child: Child,
}

impl RawAudioCapture {
    fn start(device: &str) -> Result<Self, String> {
        let path = output_path_with_extension("pcm")?;
        let file = OpenOptions::new().write(true).open(&path)
            .map_err(|e| format!("Could not open temporary audio file: {e}"))?;
        let child = match Command::new("/usr/bin/parec")
            .args(["--raw", "--format=s16le", "--rate=48000", "--channels=2", "--latency-msec=50"])
            .arg(format!("--device={device}"))
            .stdin(Stdio::null()).stdout(Stdio::from(file)).stderr(Stdio::null())
            .spawn() {
                Ok(child) => child,
                Err(error) => { let _ = fs::remove_file(&path); return Err(format!("Could not start {device} audio capture: {error}")); }
            };
        Ok(Self { path, child })
    }

    fn stop(&mut self) -> Result<(), String> {
        if self.child.try_wait().map_err(|e| e.to_string())?.is_none() {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
        let size = fs::metadata(&self.path).map_err(|e| e.to_string())?.len();
        if size == 0 || size > MAX_BYTES {
            return Err(format!("An audio source produced {size} bytes; no combined recording was saved. Check your microphone and system output devices."));
        }
        Ok(())
    }
}

impl Drop for RawAudioCapture {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = fs::remove_file(&self.path);
    }
}

struct PortalLease {
    connection: gio::DBusConnection,
    session: String,
    remote_fd: OwnedFd,
}

impl Drop for PortalLease {
    fn drop(&mut self) {
        if let Ok(path) = glib::variant::ObjectPath::try_from(self.session.as_str()) {
            let _ = self.connection.call_sync(
                Some(PORTAL), path.as_str(), "org.freedesktop.portal.Session", "Close",
                None, None, gio::DBusCallFlags::NONE, 5_000, None::<&gio::Cancellable>,
            );
        }
    }
}

// GstElement and GDBusConnection are reference-counted and safe to use from a
// different command thread. Access to this one pipeline is serialized by State.
unsafe impl Send for NativeScreen {}

impl Drop for NativeScreen {
    fn drop(&mut self) {
        trace_capture("native capture cleanup");
        // Stop the sensor on every path, including window close or a failed save.
        unsafe {
            gst_element_set_state(self.pipeline, 1); // GST_STATE_NULL
            gst_object_unref(self.pipeline);
        }
        let _ = fs::remove_file(&self.output);
        // PortalLease closes the OS grant and PipeWire descriptor afterward.
    }
}

fn token(prefix: &str) -> String {
    format!("{prefix}{}_{}", std::process::id(), TOKEN.fetch_add(1, Ordering::Relaxed))
}

fn portal_step(
    bus: &gio::DBusConnection,
    context: &glib::MainContext,
    answers: &Rc<RefCell<HashMap<String, (u32, glib::Variant)>>>,
    method: &str,
    args: &glib::Variant,
) -> Result<glib::Variant, String> {
    let response = bus.call_sync(
        Some(PORTAL), PORTAL_PATH, SCREENCAST, method, Some(args), None,
        gio::DBusCallFlags::NONE, 10_000, None::<&gio::Cancellable>,
    ).map_err(|e| format!("Screen portal {method}: {e}"))?;
    let handle: glib::variant::ObjectPath = response.child_get(0);
    let deadline = Instant::now() + Duration::from_secs(90);
    loop {
        if let Some((code, result)) = answers.borrow_mut().remove(handle.as_str()) {
            return if code == 0 { Ok(result) } else {
                Err(format!("Screen selection was cancelled or denied ({method}, code {code})."))
            };
        }
        if Instant::now() >= deadline {
            return Err(format!("Screen portal {method} did not respond within 90 seconds."));
        }
        while context.pending() { context.iteration(false); }
        std::thread::sleep(Duration::from_millis(25));
    }
}

fn options(token_value: &str) -> glib::VariantDict {
    let dict = glib::VariantDict::new(None);
    dict.insert("handle_token", token_value);
    dict
}

fn pipewire_fd_index(response: &glib::Variant) -> Result<i32, String> {
    // OpenPipeWireRemote returns `(h)`, where `h` is a D-Bus handle type.
    response.try_child_get::<glib::variant::Handle>(0)
        .map_err(|e| format!("Screen portal returned an unexpected fd type: {e}"))?
        .map(|handle| handle.0)
        .ok_or_else(|| "Screen portal returned no PipeWire file descriptor".into())
}

fn portal_stream() -> Result<(PortalLease, u32), String> {
    let context = glib::MainContext::new();
    context.with_thread_default(|| {
        let bus = gio::bus_get_sync(gio::BusType::Session, None::<&gio::Cancellable>)
            .map_err(|e| format!("Desktop portal session bus: {e}"))?;
        let answers = Rc::new(RefCell::new(HashMap::<String, (u32, glib::Variant)>::new()));
        let receiver = Rc::clone(&answers);
        let subscription = bus.signal_subscribe(
            Some(PORTAL), Some("org.freedesktop.portal.Request"), Some("Response"),
            None, None, gio::DBusSignalFlags::NONE,
            move |_, _, path, _, _, params| {
                if let (Ok(Some(code)), Some(result)) =
                    (params.try_child_get::<u32>(0), params.try_child_value(1)) {
                    receiver.borrow_mut().insert(path.to_string(), (code, result));
                }
            },
        );
        let mut opened_session: Option<String> = None;
        let result = (|| {
            let create_options = options(&token("screenrequest"));
            create_options.insert("session_handle_token", token("screensession"));
            let created = portal_step(&bus, &context, &answers, "CreateSession", &(create_options,).to_variant())?;
            let created_dict = glib::VariantDict::new(Some(&created));
            let session: String = created_dict.lookup("session_handle")
                .map_err(|e| e.to_string())?.ok_or("Screen portal returned no session handle")?;
            let session_path = glib::variant::ObjectPath::try_from(session.as_str())
                .map_err(|e| e.to_string())?;
            opened_session = Some(session.clone());

            let select_options = options(&token("sourcerequest"));
            select_options.insert("types", 3u32); // monitors or windows
            select_options.insert("multiple", false);
            select_options.insert("cursor_mode", 2u32); // embedded if supported
            portal_step(&bus, &context, &answers, "SelectSources", &(session_path.clone(), select_options).to_variant())?;

            let started = portal_step(
                &bus, &context, &answers, "Start",
                &(session_path.clone(), "", options(&token("startrequest"))).to_variant(),
            )?;
            let started_dict = glib::VariantDict::new(Some(&started));
            let streams = started_dict.lookup_value("streams", None)
                .ok_or("Screen portal returned no video stream")?;
            if streams.n_children() == 0 { return Err("Screen portal returned no video stream".into()); }
            let node: u32 = streams.child_value(0).try_child_get(0)
                .map_err(|e| format!("Screen portal returned an unexpected stream type: {e}"))?
                .ok_or("Screen portal returned a stream without a PipeWire node")?;
            let (fd_response, fd_list) = bus.call_with_unix_fd_list_sync(
                Some(PORTAL), PORTAL_PATH, SCREENCAST, "OpenPipeWireRemote",
                Some(&(session_path, glib::VariantDict::new(None)).to_variant()), None,
                gio::DBusCallFlags::NONE, 10_000, None::<&gio::UnixFDList>, None::<&gio::Cancellable>,
            ).map_err(|e| format!("Screen portal PipeWire access: {e}"))?;
            // D-Bus type `h` is a GVariant Handle, not an `i32` variant.
            // child_get::<i32> panics here even though the underlying value
            // is numerically an integer, aborting the whole Tauri process.
            let fd_index = pipewire_fd_index(&fd_response)?;
            let fd = fd_list.get(fd_index).map_err(|e| e.to_string())?;
            // Gio returns a new owned descriptor. The GStreamer pipeline keeps
            // this descriptor alive until NativeScreen is dropped.
            let owned_fd = unsafe { OwnedFd::from_raw_fd(fd) };
            Ok((PortalLease { connection: bus.clone(), session, remote_fd: owned_fd }, node))
        })();
        if result.is_err() {
            if let Some(session) = opened_session {
                let _ = bus.call_sync(
                    Some(PORTAL), session.as_str(), "org.freedesktop.portal.Session", "Close",
                    None, None, gio::DBusCallFlags::NONE, 5_000, None::<&gio::Cancellable>,
                );
            }
        }
        bus.signal_unsubscribe(subscription);
        result
    }).map_err(|e| format!("Could not initialize the screen portal context: {e}"))?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pipewire_remote_reply_uses_dbus_handle_not_integer() {
        let reply = (glib::variant::Handle(2),).to_variant();
        assert_eq!(pipewire_fd_index(&reply), Ok(2));
        assert!(pipewire_fd_index(&(2i32,).to_variant()).is_err());
    }

    #[test]
    fn live_pipeline_accepts_only_one_audio_source() {
        let screen = pipeline_description(42, 9, "/tmp/continuum-test.webm", false, false);
        assert!(!screen.contains("pulsesrc"));
        assert!(!screen.contains("mux.audio_0"));
        let microphone = pipeline_description(42, 9, "/tmp/continuum-test.webm", false, true);
        assert!(microphone.contains("device=@DEFAULT_SOURCE@"));
        assert!(!microphone.contains("audiomixer"));
        assert!(microphone.contains("opusenc bitrate=96000 ! queue ! mux.audio_0"));
        let system = pipeline_description(42, 9, "/tmp/continuum-test.webm", true, false);
        assert!(system.contains("device=@DEFAULT_MONITOR@"));
        assert!(!system.contains("audiomixer"));
    }

    #[test]
    fn abrupt_stop_and_offline_remux_finalize_one_audio_source_without_sensors() {
        let output = output_path().unwrap();
        let description = pipeline_description(7, 9, &output.display().to_string(), false, true)
            .replace(
                "pipewiresrc name=screen_source fd=7 path=9 provide-clock=false do-timestamp=true",
                "videotestsrc name=screen_source is-live=true do-timestamp=true",
            )
            .replace(
                "pulsesrc name=microphone_source device=@DEFAULT_SOURCE@ provide-clock=false do-timestamp=true",
                "audiotestsrc name=microphone_source is-live=true do-timestamp=true",
            );
        let description = CString::new(description).unwrap();
        unsafe {
            assert_ne!(gst_init_check(std::ptr::null_mut(), std::ptr::null_mut(), std::ptr::null_mut()), 0);
            let mut error = std::ptr::null_mut();
            let pipeline = gst_parse_launch(description.as_ptr(), &mut error);
            assert!(error.is_null());
            assert!(!pipeline.is_null());
            assert_ne!(gst_element_set_state(pipeline, 4), 0);
            std::thread::sleep(Duration::from_millis(500));
            gst_element_set_state(pipeline, 1);
            gst_object_unref(pipeline);
        }
        let bytes = remux_streamable(&output).unwrap();
        assert!(bytes.starts_with(&[0x1a, 0x45, 0xdf, 0xa3]));
        assert!(bytes.len() > 1_000);
        fs::remove_file(output).unwrap();
    }

    #[test]
    fn finite_screen_and_two_pcm_sources_mix_without_live_devices() {
        let video = output_path().unwrap();
        let system = output_path_with_extension("pcm").unwrap();
        let microphone = output_path_with_extension("pcm").unwrap();
        let video_description = format!(
            "videotestsrc num-buffers=30 ! videoconvert ! video/x-raw,format=I420,framerate=30/1 ! \
             vp8enc deadline=1 ! webmmux ! filesink location={}", video.display(),
        );
        let video_bytes = run_offline_mux(video_description, &video).unwrap();
        assert!(video_bytes.len() > 1_000);
        let system_pcm = 500i16.to_le_bytes().repeat(48_000 * 2);
        let microphone_pcm = 1_000i16.to_le_bytes().repeat(48_000 * 2);
        fs::write(&system, system_pcm).unwrap();
        fs::write(&microphone, microphone_pcm).unwrap();
        let result = mix_separate_audio(&video, &system, &microphone).unwrap();
        assert!(result.len() > video_bytes.len());
        assert!(result.starts_with(&[0x1a, 0x45, 0xdf, 0xa3]));
        fs::remove_file(video).unwrap();
        fs::remove_file(system).unwrap();
        fs::remove_file(microphone).unwrap();
    }
}

fn pipeline_description(fd: i32, node: u32, output: &str, system_audio: bool, microphone: bool) -> String {
    assert!(!(system_audio && microphone), "two audio sources must be recorded separately");
    let streamable = if system_audio || microphone { "streamable=true" } else { "" };
    let mut description = format!(
        "webmmux name=mux {streamable} ! filesink location={output} \
         pipewiresrc name=screen_source fd={fd} path={node} provide-clock=false do-timestamp=true ! \
         video/x-raw,format=BGRA ! videoconvert ! videorate ! \
         video/x-raw,format=I420,framerate=30/1 ! identity eos-after={MAX_FRAMES} ! \
         vp8enc deadline=1 target-bitrate=1500000 ! queue ! mux.video_0"
    );
    if system_audio || microphone {
        for (name, device) in [
            system_audio.then_some(("system_audio_source", "@DEFAULT_MONITOR@")),
            microphone.then_some(("microphone_source", "@DEFAULT_SOURCE@")),
        ].into_iter().flatten() {
            description.push_str(&format!(
                " pulsesrc name={name} device={device} provide-clock=false do-timestamp=true ! queue ! \
                 audioconvert ! audioresample ! audio/x-raw,format=S16LE,rate=48000,channels=2 ! \
                 opusenc bitrate=96000 ! queue ! mux.audio_0"
            ));
        }
    }
    description
}

fn output_path_with_extension(extension: &str) -> Result<PathBuf, String> {
    let timestamp = SystemTime::now().duration_since(UNIX_EPOCH)
        .map_err(|e| e.to_string())?.as_nanos();
    let path = std::env::temp_dir().join(format!("continuum-screen-{}-{timestamp}.{extension}", std::process::id()));
    OpenOptions::new().write(true).create_new(true).mode(0o600)
        .open(&path).map_err(|e| format!("Could not reserve temporary recording: {e}"))?;
    Ok(path)
}

fn output_path() -> Result<PathBuf, String> { output_path_with_extension("webm") }

struct TempOutput(PathBuf);

impl Drop for TempOutput {
    fn drop(&mut self) { let _ = fs::remove_file(&self.0); }
}

fn remux_streamable(input: &PathBuf) -> Result<Vec<u8>, String> {
    let output = TempOutput(output_path()?);
    // The live muxer is deliberately streamable: shutting down its Pulse
    // sources does not require a synchronous EOS call. This finite offline
    // pass restores a normal duration/index without decoding user media.
    let description = format!(
        "filesrc location={} ! matroskademux name=demux \
         demux.video_0 ! queue ! mux.video_0 \
         demux.audio_0 ! queue ! mux.audio_0 \
         webmmux name=mux ! filesink location={}",
        input.display(), output.0.display(),
    );
    run_offline_mux(description, &output.0)
}

fn mix_separate_audio(video: &PathBuf, system: &PathBuf, microphone: &PathBuf) -> Result<Vec<u8>, String> {
    let output = TempOutput(output_path()?);
    // All inputs are finite local files. The mixer never owns a live Pulse or
    // PipeWire source, so Stop cannot wait for two device clocks to converge.
    let description = format!(
        "webmmux name=mux ! filesink location={} \
         filesrc location={} ! matroskademux name=demux demux.video_0 ! queue ! mux.video_0 \
         audiomixer name=mix ignore-inactive-pads=true ! audioconvert ! audioresample ! \
         audio/x-raw,format=S16LE,rate=48000,channels=2 ! opusenc bitrate=96000 ! queue ! mux.audio_0 \
         filesrc location={} ! rawaudioparse format=pcm pcm-format=s16le sample-rate=48000 num-channels=2 ! volume volume=0.5 ! queue ! mix. \
         filesrc location={} ! rawaudioparse format=pcm pcm-format=s16le sample-rate=48000 num-channels=2 ! volume volume=0.5 ! queue ! mix.",
        output.0.display(), video.display(), system.display(), microphone.display(),
    );
    run_offline_mux(description, &output.0)
}

fn run_offline_mux(description: String, output: &PathBuf) -> Result<Vec<u8>, String> {
    let description = CString::new(description).map_err(|e| e.to_string())?;
    unsafe {
        if gst_init_check(std::ptr::null_mut(), std::ptr::null_mut(), std::ptr::null_mut()) == 0 {
            return Err("GStreamer could not initialize to finalize the recording.".into());
        }
        let mut error = std::ptr::null_mut();
        let pipeline = gst_parse_launch(description.as_ptr(), &mut error);
        if !error.is_null() {
            let message = (*error).message;
            let detail = if message.is_null() { "unknown remux error".into() }
                else { std::ffi::CStr::from_ptr(message).to_string_lossy().into_owned() };
            glib::ffi::g_error_free(error);
            if !pipeline.is_null() { gst_object_unref(pipeline); }
            return Err(format!("Could not finalize recorded audio and video: {detail}"));
        }
        if pipeline.is_null() || gst_element_set_state(pipeline, 4) == 0 {
            if !pipeline.is_null() { gst_element_set_state(pipeline, 1); gst_object_unref(pipeline); }
            return Err("Could not start offline recording finalization.".into());
        }
        let bus = gst_element_get_bus(pipeline);
        let eos = if bus.is_null() { std::ptr::null_mut() }
            else { gst_bus_timed_pop_filtered(bus, 10_000_000_000, 1) };
        if !bus.is_null() { gst_object_unref(bus); }
        if !eos.is_null() { gst_mini_object_unref(eos); }
        gst_element_set_state(pipeline, 1);
        gst_object_unref(pipeline);
        if eos.is_null() { return Err("Offline recording finalization did not finish within 10 seconds.".into()); }
    }
    let bytes = fs::read(output).map_err(|e| e.to_string())?;
    if bytes.len() as u64 > MAX_BYTES || !bytes.starts_with(&[0x1a, 0x45, 0xdf, 0xa3]) {
        return Err("Finalized WebM is invalid or larger than the 32 MiB recording limit.".into());
    }
    Ok(bytes)
}

impl NativeScreenState {
    pub fn start(&self, system_audio: bool, microphone: bool) -> Result<(), String> {
        trace_capture(&format!("start requested: system_audio={system_audio}, microphone={microphone}"));
        let mut current = self.current.lock().map_err(|_| "Screen recorder lock failed")?;
        if current.is_some() || self.ready.lock().map_err(|_| "Screen recorder lock failed")?.is_some() {
            return Err("A screen recording is already active or awaiting save.".into());
        }
        let (portal, node) = portal_stream()?;
        trace_capture("portal screen selected");
        let output = output_path()?;
        let separate_audio = system_audio && microphone;
        if separate_audio && !std::path::Path::new("/usr/bin/parec").is_file() {
            let _ = fs::remove_file(&output);
            return Err("Screen + System audio + Microphone needs the parec audio capture tool on this Linux computer.".into());
        }
        let description = pipeline_description(
            std::os::fd::AsRawFd::as_raw_fd(&portal.remote_fd), node,
            &output.display().to_string(), system_audio && !separate_audio, microphone && !separate_audio,
        );
        let description = CString::new(description).map_err(|e| e.to_string())?;
        unsafe {
            if gst_init_check(std::ptr::null_mut(), std::ptr::null_mut(), std::ptr::null_mut()) == 0 {
                return Err("GStreamer could not initialize for native screen capture.".into());
            }
            let mut error = std::ptr::null_mut();
            let pipeline = gst_parse_launch(description.as_ptr(), &mut error);
            if !error.is_null() {
                let message = (*error).message;
                let detail = if message.is_null() { "unknown pipeline error".into() }
                    else { std::ffi::CStr::from_ptr(message).to_string_lossy().into_owned() };
                glib::ffi::g_error_free(error);
                if !pipeline.is_null() { gst_object_unref(pipeline); }
                let _ = fs::remove_file(&output);
                return Err(format!("Native screen encoder cannot start: {detail}"));
            }
            if pipeline.is_null() || gst_element_set_state(pipeline, 4) == 0 { // GST_STATE_PLAYING
                if !pipeline.is_null() { gst_element_set_state(pipeline, 1); gst_object_unref(pipeline); }
                let _ = fs::remove_file(&output);
                return Err("Native screen encoder failed to start.".into());
            }
            let mut actual = 0;
            let mut pending = 0;
            if gst_element_get_state(pipeline, &mut actual, &mut pending, 3_000_000_000) == 0 {
                gst_element_set_state(pipeline, 1);
                gst_object_unref(pipeline);
                let _ = fs::remove_file(&output);
                return Err("The selected screen or audio source could not start. Check the screen chooser and Linux microphone/audio device, then retry.".into());
            }
            trace_capture("native pipeline started");
            let mut capture = NativeScreen {
                pipeline, _portal: portal, output, raw_audio: Vec::new(),
                started: Instant::now(), system_audio, microphone,
            };
            if separate_audio {
                capture.raw_audio.push(RawAudioCapture::start("@DEFAULT_MONITOR@")?);
                capture.raw_audio.push(RawAudioCapture::start("@DEFAULT_SOURCE@")?);
                trace_capture("separate system and microphone capture started");
            }
            *current = Some(capture);
        }
        Ok(())
    }

    pub fn finish(&self) -> Result<(), String> {
        trace_capture("stop requested");
        if self.ready.lock().map_err(|_| "Screen recorder lock failed")?.is_some() { return Ok(()); }
        let mut current = self.current.lock().map_err(|_| "Screen recorder lock failed")?;
        let mut capture = current.take().ok_or("There is no active native screen recording")?;
        let duration_ms = capture.started.elapsed().as_millis() as u64;
        let separate_audio = capture.system_audio && capture.microphone;
        let mut audio_error = None;
        for audio in &mut capture.raw_audio {
            if let Err(error) = audio.stop() { audio_error.get_or_insert(error); }
        }
        if (capture.system_audio || capture.microphone) && !separate_audio {
            // A synchronous EOS event on a live Pulse source can lock inside
            // audiomixer. The streamable muxer remains readable after a state
            // stop; remux it from a finite file below to restore duration.
            trace_capture("stopping streamable native pipeline");
            unsafe { gst_element_set_state(capture.pipeline, 1); }
            trace_capture("streamable native pipeline stopped");
        } else { unsafe {
                // The 60-second safety cap may already have sent EOS. Sending
                // another event can return false even though final EOS is on
                // the bus, so inspect the bus rather than discarding the file.
                let _ = gst_element_send_event(capture.pipeline, gst_event_new_eos());
                trace_capture("EOS sent");
                let bus = gst_element_get_bus(capture.pipeline);
                if bus.is_null() { return Err("Native screen encoder has no event bus.".into()); }
                let eos = gst_bus_timed_pop_filtered(bus, 5_000_000_000, 1); // GST_MESSAGE_EOS
                trace_capture("EOS wait returned");
                gst_object_unref(bus);
                if eos.is_null() { return Err("Native screen encoder did not finalize within 5 seconds.".into()); }
                // GstMessage is a GstMiniObject, not a GstObject.
                gst_mini_object_unref(eos);
                trace_capture("setting native pipeline to NULL");
                gst_element_set_state(capture.pipeline, 1);
        } }
        trace_capture("native pipeline stopped");
        if let Some(error) = audio_error { return Err(error); }
        let size = fs::metadata(&capture.output).map_err(|e| e.to_string())?.len();
        if std::env::var_os("CONTINUUM_CAPTURE_DIAGNOSTICS").is_some() {
            eprintln!("[continuum-capture] streamable file size: {size} bytes");
        }
        if size == 0 || size > MAX_BYTES {
            return Err(format!("Recording is {size} bytes; the safe limit is 32 MiB. Nothing was added to the workspace."));
        }
        let bytes = if separate_audio {
            trace_capture("mixing separate audio files offline");
            mix_separate_audio(&capture.output, &capture.raw_audio[0].path, &capture.raw_audio[1].path)?
        } else if capture.system_audio || capture.microphone {
            trace_capture("remuxing streamable recording offline");
            remux_streamable(&capture.output)?
        } else { fs::read(&capture.output).map_err(|e| e.to_string())? };
        trace_capture("recording file loaded");
        if !bytes.starts_with(&[0x1a, 0x45, 0xdf, 0xa3]) {
            return Err("Screen recorder did not create a valid WebM file.".into());
        }
        let (system_audio, microphone) = (capture.system_audio, capture.microphone);
        drop(capture); // Close the PipeWire portal and remove the private temp file.
        *self.ready.lock().map_err(|_| "Screen recorder lock failed")? = Some(RecordedScreen {
            bytes, duration_ms, system_audio, microphone,
        });
        trace_capture("recording ready to save");
        Ok(())
    }

    pub fn take_ready(&self) -> Result<Option<RecordedScreen>, String> {
        Ok(self.ready.lock().map_err(|_| "Screen recorder lock failed")?.take())
    }

    pub fn restore_ready(&self, value: RecordedScreen) -> Result<(), String> {
        *self.ready.lock().map_err(|_| "Screen recorder lock failed")? = Some(value);
        Ok(())
    }

    pub fn cancel(&self) {
        if let Ok(mut current) = self.current.lock() { current.take(); }
        if let Ok(mut ready) = self.ready.lock() { ready.take(); }
    }
}
