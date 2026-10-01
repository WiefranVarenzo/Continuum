/** Recording uses the selected input without conferencing filters. In particular,
 * echo cancellation must not decide which sounds survive a screen/audio capture. */
export function microphoneConstraints(deviceId = ""): MediaStreamConstraints {
  return {video:false, audio:{
    ...(deviceId && deviceId !== "default" ? {deviceId:{exact:deviceId}} : {}),
    echoCancellation:false, noiseSuppression:false, autoGainControl:false,
    sampleRate:{ideal:48000},
  }};
}

export interface MicrophoneLevel {
  label: string;
  level: number;
  received: boolean;
  muted: boolean;
  silent: boolean;
  suspended: boolean;
}

/** A separate, retained input graph works for mic-only and combined recording.
 * Nothing connects to the speakers. The meter observes this microphone only,
 * never the system-audio mix, so desktop sound cannot hide a silent input. */
export function microphoneAudio(stream:MediaStream, context:AudioContext, initialGain:number,
  onLevel:(sample:MicrophoneLevel)=>void) {
  const track=stream.getAudioTracks()[0];
  if(!track || track.readyState!=="live" || !track.enabled)
    throw new Error("The microphone returned no live, enabled audio track. Select another input and retry.");
  const source=context.createMediaStreamSource(new MediaStream([track]));
  const gain=context.createGain();
  const analyser=context.createAnalyser();
  const destination=context.createMediaStreamDestination();
  const nodes:AudioNode[]=[source,gain,analyser,destination];
  let closed=false, timer:ReturnType<typeof setInterval>|undefined;
  const release=()=>{
    if(closed)return;closed=true;
    if(timer)clearInterval(timer);
    nodes.forEach(node=>node.disconnect());
    destination.stream.getTracks().forEach(output=>output.stop());
  };
  const setGain=(value:number)=>{gain.gain.value=Math.max(.25,Math.min(4,Number.isFinite(value)?value:1));};
  try {
    setGain(initialGain);analyser.fftSize=2048;
    // Keep the analyser on the recorder's pulled path. A disconnected meter
    // branch can stay silent in WebViews even while the recording has audio.
    source.connect(gain);gain.connect(analyser);analyser.connect(destination);
    const samples=new Float32Array(analyser.fftSize);
    let received=false;
    const started=performance.now();
    const sample=()=>{
      if(closed)return;
      analyser.getFloatTimeDomainData(samples);
      let energy=0;for(const value of samples)energy+=value*value;
      const rms=Math.sqrt(energy/samples.length);
      const muted=track.muted || !track.enabled || track.readyState!=="live";
      if(!muted && rms>.001)received=true;
      const level=muted?0:Math.max(0,Math.min(1,(20*Math.log10(Math.max(rms,.000001))+60)/60));
      onLevel({label:track.label || "Selected microphone",level,received,muted,
        silent:!received && performance.now()-started>=6000,suspended:context.state!=="running"});
    };
    sample();timer=setInterval(sample,150);
    return {stream:destination.stream,release,setGain};
  } catch(error){release();throw error;}
}
