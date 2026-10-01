import gi, json, sys, base64, tempfile, argparse
from pathlib import Path
parser = argparse.ArgumentParser(description='Sensor-free WebKit recorder/playback integration probe')
parser.add_argument('--include-combined', action='store_true', help='Also reproduce the WebKit mixed-clock limitation')
args = parser.parse_args()
artifacts = Path(tempfile.mkdtemp(prefix='continuum-media-proof-'))
passed = False
gi.require_version('Gtk', '3.0')
gi.require_version('WebKit2', '4.1')
from gi.repository import Gtk, WebKit2, GLib

# Sensor-free integration test: only synthetic canvas frames and oscillator audio.
manager = WebKit2.UserContentManager()
manager.register_script_message_handler('result')
def result(manager, value):
    global passed
    message=value.get_js_value().to_string()
    if message.startswith('media:'):
        _, mode, data = message.split(':', 2)
        if mode in ('video','audio','combined'):
            with (artifacts / (mode+'.webm')).open('wb') as media:
                media.write(base64.b64decode(data))
        return
    print(message, flush=True)
    if not message.startswith('stage:'):
        report = json.loads(message)
        passed = bool(report.get('runs')) and all(run.get('decoded') and run.get('bytes', 0) > 0 and run.get('header') == [26,69,223,163] for run in report['runs']) and not report.get('error')
        print('Synthetic media artifacts: '+str(artifacts), flush=True)
        Gtk.main_quit()
manager.connect('script-message-received::result', result)
view = WebKit2.WebView(user_content_manager=manager, website_policies=WebKit2.WebsitePolicies(autoplay=WebKit2.AutoplayPolicy.ALLOW))
view.get_settings().set_enable_media_stream(True)
view.get_settings().set_media_playback_requires_user_gesture(False)
window = Gtk.Window()
window.add(view)
window.set_default_size(320, 240)
window.show_all()
view.load_html('''<!doctype html><canvas width="320" height="240"></canvas><script>
(async()=>{
const out={supported:{},runs:[]};
const log=m=>window.webkit.messageHandlers.result.postMessage('stage:'+m);log('script');
try {
 for(const type of ['video/webm;codecs=vp8,opus','video/webm;codecs=vp8','audio/webm;codecs=opus','video/mp4'])out.supported[type]=MediaRecorder.isTypeSupported(type);
 for(const mode of MODES){
  const c=document.querySelector('canvas'), ctx=c.getContext('2d');let tick=0;
  const timer=setInterval(()=>{ctx.fillStyle=`hsl(${tick++*10} 80% 50%)`;ctx.fillRect(0,0,320,240);},40);
  log(mode);const audio=new AudioContext({sampleRate:48000});void audio.resume();
  const oscillator=audio.createOscillator(), dest=audio.createMediaStreamDestination();oscillator.connect(dest);oscillator.start();
  const video=c.captureStream(25);
  const stream=new MediaStream([...(mode!=='audio'?video.getVideoTracks():[]),...(mode!=='video'?dest.stream.getAudioTracks():[])]);
  const type=mode==='audio'?'audio/webm;codecs=opus':mode==='video'?'video/webm;codecs=vp8':'video/webm;codecs=vp8,opus';
  const run={mode,type,bytes:0,chunks:0,audioState:audio.state};out.runs.push(run);
  try{
   const recorder=new MediaRecorder(stream,{mimeType:type});const chunks=[];
   recorder.ondataavailable=e=>{if(e.data.size){chunks.push(e.data);run.bytes+=e.data.size;run.chunks++;}};
   await new Promise((resolve,reject)=>{recorder.onstop=resolve;recorder.onerror=e=>reject(new Error(e.error.message));recorder.start(5000);setTimeout(()=>recorder.stop(),6200);});
   const blob=new Blob(chunks,{type:recorder.mimeType});
   const encoded=await new Promise(resolve=>{const reader=new FileReader();reader.onload=()=>resolve(reader.result.substring(reader.result.lastIndexOf(',')+1));reader.readAsDataURL(blob);});
   window.webkit.messageHandlers.result.postMessage('media:'+mode+':'+encoded);
   run.header=Array.from(new Uint8Array(await blob.slice(0,4).arrayBuffer()));
   const player=document.createElement(mode==='audio'?'audio':'video');document.body.append(player);player.muted=true;player.src=URL.createObjectURL(blob);run.recordedMime=recorder.mimeType;
   await new Promise((resolve,reject)=>{player.onloadeddata=resolve;player.onerror=()=>reject(new Error('decode failed'));setTimeout(()=>reject(new Error('decode timeout')),4000);});
   await player.play();await new Promise(r=>setTimeout(r,700));
   run.decoded=player.currentTime>0 && (mode==='audio'||player.videoWidth===320);run.width=player.videoWidth;run.playbackTime=player.currentTime;player.pause();URL.revokeObjectURL(player.src);
  }catch(e){run.error=e.message;}
  clearInterval(timer);video.getTracks().forEach(t=>t.stop());dest.stream.getTracks().forEach(t=>t.stop());oscillator.stop();await audio.close();
 }
}catch(e){out.error=e.message;}
window.webkit.messageHandlers.result.postMessage(JSON.stringify(out));
})();</script>'''.replace('MODES', json.dumps(['video','audio','combined'] if args.include_combined else ['video','audio'])),'http://localhost/')
def timeout():
    print('PROBE TIMEOUT', flush=True)
    Gtk.main_quit()
    return False
GLib.timeout_add_seconds(40, timeout)
Gtk.main()
sys.exit(0 if passed else 1)
