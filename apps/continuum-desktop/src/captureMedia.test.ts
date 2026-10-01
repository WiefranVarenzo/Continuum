import {afterEach,describe,it,expect,vi} from "vitest";
import {requestMediaPermission,captureError,recordingStream,relayScreen,startRecorder} from "./captureMedia";
afterEach(()=>{vi.useRealTimers();vi.unstubAllGlobals();});
describe("Capture permission lifecycle",()=>{
 it("releases a late OS grant after the user cancels",async()=>{
  let grant!:(stream:MediaStream)=>void;const stop=vi.fn();const controller=new AbortController();
  const request=requestMediaPermission(new Promise(resolve=>grant=resolve),"screen",controller.signal);
  controller.abort();await expect(request).rejects.toThrow("cancelled");
  grant({getTracks:()=>[{stop}]} as unknown as MediaStream);await Promise.resolve();expect(stop).toHaveBeenCalledOnce();
 });
 it("times out and releases a late stream, rather than leaving sensors running",async()=>{
  vi.useFakeTimers();let grant!:(stream:MediaStream)=>void;const stop=vi.fn();
  const request=requestMediaPermission(new Promise(resolve=>grant=resolve),"screen");
  const failure=expect(request).rejects.toThrow("90 seconds");await vi.advanceTimersByTimeAsync(90000);await failure;
  grant({getTracks:()=>[{stop}]} as unknown as MediaStream);await Promise.resolve();expect(stop).toHaveBeenCalledOnce();
 });
 it("explains the distinction between denied permission and missing user activation",()=>{
  expect(captureError(new DOMException("blocked","NotAllowedError"))).toContain("permission");
  expect(captureError(new DOMException("activation","InvalidStateError"))).toContain("directly from your click");
  expect(captureError(new DOMException("unsupported","NotSupportedError"))).toContain("encoder");
  expect(captureError(new DOMException("busy","NotReadableError"),"windows")).toContain("Windows Privacy");
  expect(captureError(new DOMException("unsupported","NotSupportedError"),"windows")).toContain("WebView2");
 });
 it("mixes microphone and desktop audio into one recorder track and releases it",async()=>{
  const close=vi.fn(),stop=vi.fn(),connect=vi.fn(),disconnect=vi.fn();const mixed={kind:"audio",stop};
  vi.stubGlobal("MediaStream",class {constructor(public tracks:MediaStreamTrack[]){}getTracks(){return this.tracks;}getAudioTracks(){return this.tracks.filter(t=>t.kind==="audio");}});
  vi.stubGlobal("AudioContext",class {close=close;resume=vi.fn().mockResolvedValue(undefined);createMediaStreamDestination(){return {disconnect,stream:{getTracks:()=>[mixed],getAudioTracks:()=>[mixed]}};}createMediaStreamSource(){return {connect,disconnect};}});
  const result=await recordingStream([{kind:"video"},{kind:"audio"},{kind:"audio"}] as MediaStreamTrack[]);
  expect(result.stream.getTracks()).toHaveLength(2);expect(connect).toHaveBeenCalledTimes(2);result.release();result.release();expect(disconnect).toHaveBeenCalledTimes(3);expect(stop).toHaveBeenCalledOnce();expect(close).toHaveBeenCalledOnce();
 });
});

describe("Recorder startup proof",()=>{
 function recorder(){return Object.assign(new EventTarget(),{start:vi.fn()}) as unknown as MediaRecorder;}
 function chunk(target:MediaRecorder, size:number){target.dispatchEvent(Object.assign(new Event("dataavailable"),{data:{size}}));}
 it("waits for nonempty encoded data, not just a start event",async()=>{
  const target=recorder();const done=vi.fn();const promise=startRecorder(target,new AbortController().signal).then(done);
  target.dispatchEvent(new Event("start"));chunk(target,0);await Promise.resolve();expect(done).not.toHaveBeenCalled();
  chunk(target,100);await promise;expect(done).toHaveBeenCalledOnce();expect(target.start).toHaveBeenCalledWith(5000);
 });
 it("fails boundedly when the encoder silently produces nothing",async()=>{
  vi.useFakeTimers();const result=startRecorder(recorder(),new AbortController().signal);
  const failure=expect(result).rejects.toThrow("not a permission denial");await vi.advanceTimersByTimeAsync(15000);await failure;
 });
 it("can cancel preparing and removes the watchdog",async()=>{
  vi.useFakeTimers();const controller=new AbortController();const result=startRecorder(recorder(),controller.signal);
  controller.abort();await expect(result).rejects.toThrow("cancelled");expect(vi.getTimerCount()).toBe(0);
 });
 it("handles unsupported constructor/start and recorder error without hanging",async()=>{
  const target=recorder();vi.mocked(target.start).mockImplementation(()=>{throw new DOMException("unsupported","NotSupportedError");});
  await expect(startRecorder(target,new AbortController().signal)).rejects.toThrow("unsupported");
  const other=recorder();const result=startRecorder(other,new AbortController().signal);other.dispatchEvent(new Event("error"));
  await expect(result).rejects.toThrow("failed to start");
 });
});

describe("User-approved screen relay",()=>{
 it("encodes a canvas track and releases the preview and timer on cancel",async()=>{
  const original=document.createElement.bind(document),video=original("video"),canvas=original("canvas");
  const draw=vi.fn(),stop=vi.fn();
  Object.defineProperty(video,"videoWidth",{value:1280});Object.defineProperty(video,"videoHeight",{value:720});
  Object.defineProperty(video,"readyState",{value:HTMLMediaElement.HAVE_CURRENT_DATA});
  vi.spyOn(video,"play").mockResolvedValue(undefined);vi.spyOn(video,"pause").mockImplementation(()=>{});
  vi.spyOn(canvas,"getContext").mockReturnValue({drawImage:draw} as unknown as CanvasRenderingContext2D);
  Object.defineProperty(canvas,"captureStream",{value:vi.fn(()=>({getTracks:()=>[{kind:"video",stop}]}))});
  vi.spyOn(document,"createElement").mockImplementation((tag,...args)=>tag==="video"?video:tag==="canvas"?canvas:original(tag,...args));
  const controller=new AbortController();
  const result=await relayScreen({getTracks:()=>[]} as unknown as MediaStream,controller.signal);
  expect(result.stream.getTracks()).toHaveLength(1);expect(draw).toHaveBeenCalled();expect(video.isConnected).toBe(true);
  controller.abort();expect(stop).toHaveBeenCalledOnce();expect(video.isConnected).toBe(false);
  result.release();expect(stop).toHaveBeenCalledOnce();
 });
});
