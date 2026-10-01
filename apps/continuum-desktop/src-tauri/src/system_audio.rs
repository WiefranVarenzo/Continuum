//! Linux desktop-output capture, explicitly started by the user. A single acknowledged
//! 50 ms packet is in flight; an unresponsive/closed WebView stops the recorder.
use super::*;
use std::io::Read;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Condvar, atomic::{AtomicBool, Ordering}};
use std::time::Duration;
use tauri::ipc::Channel;

#[derive(Default)]
pub struct SystemAudioState(Mutex<Option<RunningAudio>>);
struct RunningAudio { id:String, control:Arc<Control> }
struct Control { stopped:AtomicBool, child:Mutex<Child>, ack:Mutex<u64>, wake:Condvar }
impl Control {
    fn stop(&self) {
        self.stopped.store(true, Ordering::SeqCst);
        self.wake.notify_all();
        if let Ok(mut child) = self.child.lock() { let _=child.kill(); let _=child.wait(); }
    }
}
impl Drop for RunningAudio { fn drop(&mut self) { self.control.stop(); } }
#[derive(Clone, Serialize)]
pub struct AudioPacket { id:String, sequence:u64, samples:Vec<i16>, error:Option<String> }

pub fn available() -> bool { cfg!(target_os="linux") && Path::new("/usr/bin/parec").is_file() }

#[tauri::command]
pub fn start_system_audio(project_id:String, consent:bool, on_data:Channel<AudioPacket>, state:State<'_,AppState>, audio:State<'_,SystemAudioState>) -> Result<String,String> {
    if !consent { return Err("Confirm capture consent before recording system audio.".into()); }
    with_store(&state,|store| {
        if store.manifest().project_id!=project_id || !store.capability_enabled(Space::Research)? {
            return Err(CoreError::Validation("Open the selected Research project before recording.".into()));
        }
        Ok(())
    })?;
    if !available() { return Err("System audio needs the PulseAudio client tools (parec) on this Linux desktop.".into()); }
    let mut slot=audio.0.lock().map_err(|_|"Audio state unavailable")?;
    if slot.as_ref().is_some_and(|running|!running.control.stopped.load(Ordering::SeqCst)) {return Err("System audio is already recording.".into());}
    let mut child=Command::new("/usr/bin/parec").args(["--raw","--device=@DEFAULT_MONITOR@","--format=s16le","--rate=48000","--channels=2","--latency-msec=50"])
        .stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn().map_err(|e|format!("Could not start desktop audio: {e}"))?;
    let Some(mut stdout)=child.stdout.take() else {let _=child.kill();let _=child.wait();return Err("Desktop audio stream unavailable".into());};
    let id=continuum_core::new_id();
    let control=Arc::new(Control{stopped:AtomicBool::new(false),child:Mutex::new(child),ack:Mutex::new(0),wake:Condvar::new()});
    *slot=Some(RunningAudio{id:id.clone(),control:control.clone()});
    let packet_id=id.clone();
    let worker_control=control.clone();
    std::thread::spawn(move || {
        let mut sequence=0u64;
        let mut buffer=[0u8;9600]; // 2,400 stereo frames at 48 kHz.
        let mut failure=None;
        while !worker_control.stopped.load(Ordering::SeqCst) {
            if stdout.read_exact(&mut buffer).is_err() {failure=Some("Desktop audio stopped. Check your output device and PipeWire audio service.".to_string());break;}
            sequence+=1;
            let samples=buffer.chunks_exact(2).map(|b|i16::from_le_bytes([b[0],b[1]])).collect();
            if on_data.send(AudioPacket{id:packet_id.clone(),sequence,samples,error:None}).is_err(){break;}
            let Ok(ack)=worker_control.ack.lock() else {break;};
            let Ok((ack,wait))=worker_control.wake.wait_timeout_while(ack,Duration::from_secs(3),|ack| *ack<sequence && !worker_control.stopped.load(Ordering::SeqCst)) else {break;};
            if wait.timed_out() && *ack<sequence {failure=Some("System audio stopped because its recording buffer could not keep up. Saved capture segments remain available.".into());break;}
        }
        if !worker_control.stopped.load(Ordering::SeqCst) {
            let _=on_data.send(AudioPacket{id:packet_id,sequence,samples:vec![],error:Some(failure.unwrap_or_else(||"System audio connection closed.".into()))});
        }
        worker_control.stop();
    });
    // Release even if the audio server stops producing data while read_exact blocks.
    std::thread::spawn(move || {
        let mut previous=0;
        while !control.stopped.load(Ordering::SeqCst) {
            std::thread::sleep(Duration::from_secs(5));
            let current=control.ack.lock().map(|ack|*ack).unwrap_or(previous);
            if current==previous {control.stop();break;}
            previous=current;
        }
    });
    Ok(id)
}

#[tauri::command]
pub fn acknowledge_system_audio(id:String, sequence:u64, audio:State<'_,SystemAudioState>) -> Result<(),String> {
    let slot=audio.0.lock().map_err(|_|"Audio state unavailable")?;
    if let Some(running)=slot.as_ref().filter(|r|r.id==id) {
        let mut ack=running.control.ack.lock().map_err(|_|"Audio buffer unavailable")?;
        *ack=(*ack).max(sequence);running.control.wake.notify_all();
    }
    Ok(())
}

#[tauri::command]
pub fn stop_system_audio(id:String, audio:State<'_,SystemAudioState>) -> Result<(),String> {
    let mut slot=audio.0.lock().map_err(|_|"Audio state unavailable")?;
    if slot.as_ref().is_some_and(|r|r.id==id) {slot.take();}
    Ok(())
}

impl SystemAudioState { pub fn stop(&self) {if let Ok(mut slot)=self.0.lock(){slot.take();}} }
