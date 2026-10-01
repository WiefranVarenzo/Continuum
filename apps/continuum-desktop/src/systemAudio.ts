import {Channel, invoke} from "@tauri-apps/api/core";
interface AudioPacket {id:string;sequence:number;samples:number[];error:string|null}

export async function desktopAudio(projectId:string, signal:AbortSignal, onError:(error:Error)=>void, preparedContext?:AudioContext) {
  const context=preparedContext??new AudioContext({sampleRate:48000});
  const destination=context.createMediaStreamDestination();
  let id="", closed=false, nextTime=0, received=false;
  let lastPacket=Date.now();
  let readyResolve!:()=>void,readyReject!:(error:Error)=>void;
  const ready=new Promise<void>((resolve,reject)=>{readyResolve=resolve;readyReject=reject;});
  // The native call may still be starting when cancellation occurs.
  void ready.catch(()=>{});
  const release=()=>{if(closed)return;closed=true;clearTimeout(timer);clearInterval(watchdog);signal.removeEventListener("abort",abort);destination.stream.getTracks().forEach(t=>t.stop());if(!preparedContext)void context.close();if(id)void invoke("stop_system_audio",{id}).catch(()=>{});};
  const fail=(error:Error)=>{if(closed)return;readyReject(error);release();if(received)onError(error);};
  const abort=()=>fail(new DOMException("Capture cancelled.","AbortError"));
  const timer=setTimeout(()=>fail(new Error("Desktop audio did not start. Check your output device and PipeWire audio service.")),12_000);
  const watchdog=setInterval(()=>{if(received&&Date.now()-lastPacket>3000)fail(new Error("Desktop audio stopped responding. Recording stopped; saved segments remain available."));},1000);
  signal.addEventListener("abort",abort,{once:true});
  const onData=new Channel<AudioPacket>();
  onData.onmessage=packet=>{
    id=packet.id;
    if(closed){void invoke("stop_system_audio",{id}).catch(()=>{});return;}
    if(packet.error){fail(new Error(packet.error));return;}
    try {
      lastPacket=Date.now();
      if(packet.samples.length!==4800)throw new Error("Invalid desktop audio packet; recording stopped safely.");
      const now=context.currentTime;
      if(received && (nextTime>now+0.5 || nextTime<now-0.2))throw new Error("System audio could not stay synchronized. Recording stopped; saved segments remain available.");
      const frames=packet.samples.length/2;
      const buffer=context.createBuffer(2,frames,48000);
      for(let channel=0;channel<2;channel++){const data=buffer.getChannelData(channel);for(let i=0;i<frames;i++)data[i]=packet.samples[i*2+channel]/32768;}
      const source=context.createBufferSource();source.buffer=buffer;source.connect(destination);
      nextTime=Math.max(nextTime,now+0.08);source.start(nextTime);nextTime+=frames/48000;
      source.onended=()=>source.disconnect();
      received=true;clearTimeout(timer);readyResolve();
      void invoke("acknowledge_system_audio",{id,sequence:packet.sequence}).catch(error=>fail(new Error(String(error))));
    } catch(error){fail(error instanceof Error?error:new Error(String(error)));}
  };
  try {
    if(signal.aborted)throw new DOMException("Capture cancelled.","AbortError");
    await context.resume();
    id=await invoke<string>("start_system_audio",{projectId,consent:true,onData});
    if(closed){await invoke("stop_system_audio",{id});throw new DOMException("Capture cancelled.","AbortError");}
    await ready;
    return {stream:destination.stream,release};
  }catch(error){release();throw error;}
}
