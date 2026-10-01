import {afterEach,beforeEach,it,expect,vi} from "vitest";
import {desktopAudio} from "./systemAudio";
const bridge=vi.hoisted(()=>({invoke:vi.fn(),handler:null as null|{onmessage:(packet:unknown)=>void}}));
vi.mock("@tauri-apps/api/core",()=>({invoke:bridge.invoke,Channel:class {onmessage=()=>{};constructor(){bridge.handler=this;}}}));
const stop=vi.fn(),close=vi.fn();
beforeEach(()=>{
 vi.useFakeTimers();stop.mockClear();close.mockClear();bridge.invoke.mockReset();
 bridge.invoke.mockImplementation(async(command)=>command==="start_system_audio"?"audio1":undefined);
 vi.stubGlobal("AudioContext",class {currentTime=0;resume=vi.fn().mockResolvedValue(undefined);close=close;
 createMediaStreamDestination(){return {stream:{getTracks:()=>[{stop}]}};}
 createBuffer(){return {getChannelData:()=>new Float32Array(2400)};}
 createBufferSource(){return {connect:vi.fn(),start:vi.fn(),disconnect:vi.fn(),buffer:null,onended:null};}
 });
});
afterEach(()=>{vi.useRealTimers();vi.unstubAllGlobals();});
it("acknowledges bounded packets then stops when delivery stalls",async()=>{
 const onError=vi.fn();const pending=desktopAudio("project",new AbortController().signal,onError);
 await vi.advanceTimersByTimeAsync(0);
 bridge.handler!.onmessage({id:"audio1",sequence:1,samples:Array(4800).fill(0),error:null});
 await pending;expect(bridge.invoke).toHaveBeenCalledWith("acknowledge_system_audio",{id:"audio1",sequence:1});
 await vi.advanceTimersByTimeAsync(4000);
 expect(onError).toHaveBeenCalledOnce();expect(stop).toHaveBeenCalledOnce();expect(close).toHaveBeenCalledOnce();
 expect(bridge.invoke).toHaveBeenCalledWith("stop_system_audio",{id:"audio1"});
});
it("releases a pending audio source on cancellation without calling it a recording",async()=>{
 const controller=new AbortController();const onError=vi.fn();const pending=desktopAudio("project",controller.signal,onError);
 const failure=expect(pending).rejects.toThrow("cancelled");await vi.advanceTimersByTimeAsync(0);controller.abort();await failure;
 expect(stop).toHaveBeenCalledOnce();expect(onError).not.toHaveBeenCalled();
 expect(bridge.invoke).toHaveBeenCalledWith("stop_system_audio",{id:"audio1"});
});
