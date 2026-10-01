//! Explicit-consent Windows WASAPI loopback. No virtual cable or capture driver.
//! COM objects live and are released on the capture thread. The WebView receives
//! bounded, acknowledged 50 ms stereo packets at 48 kHz, matching the Linux API.
use super::*;
use std::collections::VecDeque;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Condvar,
};
use std::time::{Duration, Instant};
use tauri::ipc::Channel;
use windows::Win32::{Media::Audio::*, System::Com::*};

const PACKET_SAMPLES: usize = 4800;
const MAX_QUEUED_SAMPLES: usize = 288000; // At most three seconds; no unbounded backlog.

#[derive(Default)]
pub struct SystemAudioState(Mutex<Option<RunningAudio>>);
struct RunningAudio {
    id: String,
    control: Arc<Control>,
}
struct Control {
    stopped: AtomicBool,
    ack: Mutex<u64>,
    wake: Condvar,
}
impl Control {
    fn stop(&self) {
        self.stopped.store(true, Ordering::SeqCst);
        self.wake.notify_all();
    }
}
impl Drop for RunningAudio {
    fn drop(&mut self) {
        self.control.stop();
    }
}
#[derive(Clone, Serialize)]
pub struct AudioPacket {
    id: String,
    sequence: u64,
    samples: Vec<i16>,
    error: Option<String>,
}

struct ComApartment;
impl ComApartment {
    fn new() -> windows::core::Result<Self> {
        unsafe {
            CoInitializeEx(None, COINIT_MULTITHREADED).ok()?;
        }
        Ok(Self)
    }
}
impl Drop for ComApartment {
    fn drop(&mut self) {
        unsafe {
            CoUninitialize();
        }
    }
}

struct StartedClient(IAudioClient);
impl Drop for StartedClient {
    fn drop(&mut self) {
        let _ = unsafe { self.0.Stop() };
    }
}

fn initialize_loopback() -> windows::core::Result<StartedClient> {
    let enumerator: IMMDeviceEnumerator =
        unsafe { CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)? };
    let device = unsafe { enumerator.GetDefaultAudioEndpoint(eRender, eConsole)? };
    let client: IAudioClient = unsafe { device.Activate(CLSCTX_ALL, None)? };
    let format = WAVEFORMATEX {
        wFormatTag: 1,
        nChannels: 2,
        nSamplesPerSec: 48000,
        nAvgBytesPerSec: 192000,
        nBlockAlign: 4,
        wBitsPerSample: 16,
        cbSize: 0,
    };
    unsafe {
        client.Initialize(
            AUDCLNT_SHAREMODE_SHARED,
            AUDCLNT_STREAMFLAGS_LOOPBACK
                | AUDCLNT_STREAMFLAGS_AUTOCONVERTPCM
                | AUDCLNT_STREAMFLAGS_SRC_DEFAULT_QUALITY,
            1_000_000,
            0,
            &format,
            None,
        )?;
        client.Start()?;
    }
    Ok(StartedClient(client))
}

pub fn available() -> bool {
    // Query the endpoint on its own COM thread; this opens no capture stream.
    std::thread::spawn(|| -> windows::core::Result<()> {
        let _com = ComApartment::new()?;
        let enumerator: IMMDeviceEnumerator =
            unsafe { CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)? };
        let _ = unsafe { enumerator.GetDefaultAudioEndpoint(eRender, eConsole)? };
        Ok(())
    })
    .join()
    .is_ok_and(|result| result.is_ok())
}

fn endpoint_error(error: windows::core::Error) -> String {
    format!(
        "Windows system audio could not continue: {error}. Check your default output device, then restart capture. Saved segments remain available."
    )
}

fn read_available(capture: &IAudioCaptureClient, queue: &mut VecDeque<i16>) -> Result<(), String> {
    unsafe {
        while capture.GetNextPacketSize().map_err(endpoint_error)? > 0 {
            let mut data = std::ptr::null_mut();
            let mut frames = 0;
            let mut flags = 0;
            capture
                .GetBuffer(&mut data, &mut frames, &mut flags, None, None)
                .map_err(endpoint_error)?;
            let count = frames as usize * 2;
            // Always release the WASAPI buffer, including malformed or oversized data.
            let valid = count <= MAX_QUEUED_SAMPLES && queue.len() + count <= MAX_QUEUED_SAMPLES;
            if valid {
                if flags & AUDCLNT_BUFFERFLAGS_SILENT.0 as u32 != 0 {
                    queue.extend(std::iter::repeat_n(0, count));
                } else if !data.is_null() {
                    // Native PCM16 is little-endian; avoid assuming pointer alignment.
                    let bytes = std::slice::from_raw_parts(data, count * 2);
                    queue.extend(
                        bytes
                            .chunks_exact(2)
                            .map(|pair| i16::from_le_bytes([pair[0], pair[1]])),
                    );
                }
            }
            capture.ReleaseBuffer(frames).map_err(endpoint_error)?;
            if !valid {
                return Err("Windows audio buffer could not keep up. Recording stopped; saved segments remain available.".into());
            }
        }
    }
    Ok(())
}

fn capture_packets(
    client: &StartedClient,
    id: &str,
    channel: &Channel<AudioPacket>,
    control: &Control,
) -> Result<(), String> {
    let capture: IAudioCaptureClient = unsafe { client.0.GetService() }.map_err(endpoint_error)?;
    let mut queue = VecDeque::new();
    let mut sequence = 0;
    let mut deadline = Instant::now() + Duration::from_millis(50);
    while !control.stopped.load(Ordering::SeqCst) {
        read_available(&capture, &mut queue)?;
        if Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(5));
            continue;
        }
        // Short UI/IPC stalls are recoverable. The acknowledged transport, bounded
        // native queue and render-thread jitter buffer limit backlog independently.
        // WASAPI may produce no packets while the output is idle. Silence is
        // part of the output timeline, so keep it synchronized with the video.
        let mut samples: Vec<i16> = queue.drain(..queue.len().min(PACKET_SAMPLES)).collect();
        samples.resize(PACKET_SAMPLES, 0);
        sequence += 1;
        channel
            .send(AudioPacket {
                id: id.into(),
                sequence,
                samples,
                error: None,
            })
            .map_err(|_| "System audio connection closed.".to_string())?;
        let ack = control.ack.lock().map_err(|_| "Audio buffer unavailable")?;
        let (ack, timeout) = control
            .wake
            .wait_timeout_while(ack, Duration::from_secs(3), |ack| {
                *ack < sequence && !control.stopped.load(Ordering::SeqCst)
            })
            .map_err(|_| "Audio buffer unavailable")?;
        if timeout.timed_out() && *ack < sequence {
            return Err("System audio stopped because its recording buffer could not keep up. Saved segments remain available.".into());
        }
        deadline += Duration::from_millis(50);
    }
    Ok(())
}

#[tauri::command]
pub fn start_system_audio(
    project_id: String,
    consent: bool,
    on_data: Channel<AudioPacket>,
    state: State<'_, AppState>,
    audio: State<'_, SystemAudioState>,
) -> Result<String, String> {
    if !consent {
        return Err("Confirm capture consent before recording system audio.".into());
    }
    with_store(&state, |store| {
        if store.manifest().project_id != project_id
            || !store.capability_enabled(Space::Research)?
        {
            return Err(CoreError::Validation(
                "Open the selected Research project before recording.".into(),
            ));
        }
        Ok(())
    })?;
    let mut slot = audio.0.lock().map_err(|_| "Audio state unavailable")?;
    if slot
        .as_ref()
        .is_some_and(|running| !running.control.stopped.load(Ordering::SeqCst))
    {
        return Err("System audio is already recording.".into());
    }
    let id = continuum_core::new_id();
    let control = Arc::new(Control {
        stopped: AtomicBool::new(false),
        ack: Mutex::new(0),
        wake: Condvar::new(),
    });
    let worker = control.clone();
    let packet_id = id.clone();
    let (ready_tx, ready_rx) = std::sync::mpsc::sync_channel(1);
    std::thread::spawn(move || {
        let result = (|| {
            let _com = ComApartment::new().map_err(endpoint_error)?;
            let client = initialize_loopback().map_err(endpoint_error)?;
            if ready_tx.send(Ok(())).is_err() {
                return Ok(());
            }
            capture_packets(&client, &packet_id, &on_data, &worker)
        })();
        if let Err(error) = result {
            let _ = ready_tx.try_send(Err(error.clone()));
            if !worker.stopped.load(Ordering::SeqCst) {
                let _ = on_data.send(AudioPacket {
                    id: packet_id,
                    sequence: 0,
                    samples: vec![],
                    error: Some(error),
                });
            }
        }
        worker.stop();
    });
    match ready_rx.recv_timeout(Duration::from_secs(5)) {
        Ok(Ok(())) => {
            *slot = Some(RunningAudio {
                id: id.clone(),
                control,
            });
            Ok(id)
        }
        Ok(Err(error)) => {
            control.stop();
            Err(error)
        }
        Err(_) => {
            control.stop();
            Err("Windows audio initialization timed out. Check your default output device.".into())
        }
    }
}

#[tauri::command]
pub fn acknowledge_system_audio(
    id: String,
    sequence: u64,
    audio: State<'_, SystemAudioState>,
) -> Result<(), String> {
    let slot = audio.0.lock().map_err(|_| "Audio state unavailable")?;
    if let Some(running) = slot.as_ref().filter(|running| running.id == id) {
        let mut ack = running
            .control
            .ack
            .lock()
            .map_err(|_| "Audio buffer unavailable")?;
        *ack = (*ack).max(sequence);
        running.control.wake.notify_all();
    }
    Ok(())
}

#[tauri::command]
pub fn stop_system_audio(id: String, audio: State<'_, SystemAudioState>) -> Result<(), String> {
    let mut slot = audio.0.lock().map_err(|_| "Audio state unavailable")?;
    if slot.as_ref().is_some_and(|running| running.id == id) {
        slot.take();
    }
    Ok(())
}
impl SystemAudioState {
    pub fn stop(&self) {
        if let Ok(mut slot) = self.0.lock() {
            slot.take();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "Needs an active Windows output device; plays a generated non-sensitive test tone."]
    fn wasapi_captures_generated_output_and_can_restart() {
        use windows::core::PCWSTR;
        let fixture = tempfile::tempdir().unwrap();
        let path = fixture.path().join("continuum-loopback-test.wav");
        let frames = 48000u32;
        let mut wave = Vec::new();
        wave.extend(b"RIFF");
        wave.extend((36 + frames * 4).to_le_bytes());
        wave.extend(b"WAVEfmt ");
        wave.extend(16u32.to_le_bytes());
        wave.extend(1u16.to_le_bytes());
        wave.extend(2u16.to_le_bytes());
        wave.extend(48000u32.to_le_bytes());
        wave.extend(192000u32.to_le_bytes());
        wave.extend(4u16.to_le_bytes());
        wave.extend(16u16.to_le_bytes());
        wave.extend(b"data");
        wave.extend((frames * 4).to_le_bytes());
        for frame in 0..frames {
            let sample =
                ((frame as f64 * 440.0 * std::f64::consts::TAU / 48000.0).sin() * 8192.0) as i16;
            wave.extend(sample.to_le_bytes());
            wave.extend(sample.to_le_bytes());
        }
        std::fs::write(&path, wave).unwrap();
        let wide: Vec<u16> = path
            .to_string_lossy()
            .encode_utf16()
            .chain(Some(0))
            .collect();
        let _com = ComApartment::new().expect("Windows COM unavailable");
        let client = initialize_loopback().expect("Windows loopback output device unavailable");
        struct StopTone;
        impl Drop for StopTone {
            fn drop(&mut self) {
                unsafe {
                    let _ = PlaySoundW(PCWSTR::null(), None, SND_FLAGS(0));
                }
            }
        }
        let _tone = StopTone;
        assert!(unsafe {
            PlaySoundW(
                PCWSTR(wide.as_ptr()),
                None,
                SND_FILENAME | SND_ASYNC | SND_LOOP | SND_NODEFAULT,
            )
        }
        .as_bool());
        let capture: IAudioCaptureClient = unsafe { client.0.GetService() }.unwrap();
        let mut queue = VecDeque::new();
        let mut audible = false;
        for _ in 0..100 {
            std::thread::sleep(Duration::from_millis(10));
            read_available(&capture, &mut queue).unwrap();
            audible |= queue.iter().any(|sample| sample.unsigned_abs() > 256);
            queue.clear();
        }
        assert!(
            audible,
            "The loopback stream did not capture the generated test tone"
        );
        drop(capture);
        drop(client);
        let restarted = initialize_loopback().expect("Loopback could not restart after stopping");
        drop(restarted);
    }
}
