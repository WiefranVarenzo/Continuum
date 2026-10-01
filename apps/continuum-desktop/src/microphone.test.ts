import {afterEach,describe,expect,it,vi} from "vitest";
import {microphoneAudio,microphoneConstraints,type MicrophoneLevel} from "./microphone";

afterEach(()=>{vi.useRealTimers();vi.unstubAllGlobals();});
describe("Microphone recording input",()=>{
 it("requests unfiltered input and does not silently substitute another selected device",()=>{
  expect(microphoneConstraints().audio).toMatchObject({echoCancellation:false,noiseSuppression:false,autoGainControl:false});
  expect(microphoneConstraints("usb").audio).toMatchObject({deviceId:{exact:"usb"}});
  expect(microphoneConstraints("default").audio).not.toHaveProperty("deviceId");
 });
 function fixture(){
  vi.useFakeTimers();
  const disconnect=vi.fn(),stop=vi.fn();
  const track={kind:"audio",readyState:"live",enabled:true,muted:false,label:"USB microphone"};
  const output={kind:"audio",stop};
  vi.stubGlobal("MediaStream",class {constructor(public tracks:MediaStreamTrack[]){}getTracks(){return this.tracks;}getAudioTracks(){return this.tracks;}});
  let amplitude=0;
  const source={connect:vi.fn(),disconnect};
  const gain={gain:{value:1},connect:vi.fn(),disconnect};
  const analyser={fftSize:0,getFloatTimeDomainData:vi.fn((samples:Float32Array)=>samples.fill(amplitude)),connect:vi.fn(),disconnect};
  const destination={stream:{getTracks:()=>[output],getAudioTracks:()=>[output]},disconnect};
  const context={state:"running",createMediaStreamSource:vi.fn(()=>source),createGain:()=>gain,createAnalyser:()=>analyser,createMediaStreamDestination:()=>destination,close:vi.fn()};
  const samples:MicrophoneLevel[]=[];
  const input=microphoneAudio({getAudioTracks:()=>[track]} as unknown as MediaStream,context as unknown as AudioContext,1,sample=>samples.push(sample));
  return {input,context,track,samples,gain,source,analyser,destination,disconnect,stop,setAmplitude:(value:number)=>{amplitude=value;}};
 }
 it("detects silent input separately from permission and desktop audio",async()=>{
  const f=fixture();expect(f.samples.at(-1)?.silent).toBe(false);
  await vi.advanceTimersByTimeAsync(6100);expect(f.samples.at(-1)).toMatchObject({silent:true,received:false,level:0});
  f.setAmplitude(.1);await vi.advanceTimersByTimeAsync(150);
  expect(f.samples.at(-1)).toMatchObject({silent:false,received:true,label:"USB microphone"});
  expect(f.samples.at(-1)!.level).toBeGreaterThan(.5);f.input.release();
 });
 it("reports a muted source even if the analyser last contained sound",async()=>{
  const f=fixture();f.setAmplitude(.2);f.track.muted=true;
  await vi.advanceTimersByTimeAsync(150);expect(f.samples.at(-1)).toMatchObject({muted:true,level:0,received:false});f.input.release();
 });
 it("routes gain to recorded audio, releases all nodes and preserves the shared context",()=>{
  const f=fixture();f.input.setGain(2);expect(f.gain.gain.value).toBe(2);
  expect(f.gain.connect).toHaveBeenCalledWith(f.analyser);expect(f.analyser.connect).toHaveBeenCalledWith(f.destination);
  f.input.release();f.input.release();expect(f.disconnect).toHaveBeenCalledTimes(4);expect(f.stop).toHaveBeenCalledOnce();expect(f.context.close).not.toHaveBeenCalled();expect(vi.getTimerCount()).toBe(0);
 });
 it("rejects missing and stopped microphone tracks before creating a graph",()=>{
  expect(()=>microphoneAudio({getAudioTracks:()=>[]} as unknown as MediaStream,{} as AudioContext,1,()=>{})).toThrow("live, enabled");
  expect(()=>microphoneAudio({getAudioTracks:()=>[{readyState:"ended",enabled:true}]} as unknown as MediaStream,{} as AudioContext,1,()=>{})).toThrow("live, enabled");
 });
});
