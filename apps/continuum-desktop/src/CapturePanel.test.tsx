import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { CapturePanel } from "./CapturePanel";
import {invoke} from "@tauri-apps/api/core";
import {desktopAudio} from "./systemAudio";
import {microphoneConstraints,microphoneAudio} from "./microphone";
vi.mock("./microphone",async(original)=>({...await original<typeof import("./microphone")>(),microphoneAudio:vi.fn()}));

vi.mock("./systemAudio",()=>({desktopAudio:vi.fn()}));
vi.mock("./captureMedia",async(importOriginal)=>({
  ...await importOriginal<typeof import("./captureMedia")>(),
  relayScreen:vi.fn(async(stream:MediaStream)=>({stream,release:()=>{}})),
}));

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));
afterEach(()=>vi.unstubAllGlobals());
beforeEach(()=>{
  vi.mocked(invoke).mockReset().mockImplementation(async(command)=>["list_active_research_sessions","list_capture_sessions"].includes(command)?{items:[],next_offset:null}:command==="capture_runtime_health"?{
    platform: "linux",
    native_screen_available: true,
    native_screen_audio_available: true,
    scanner_ready: true,
    pipewire_socket_ready: true,
    desktop_portal_session_ready: true,
    summary: "Native screen runtime is ready.",
  }:undefined);
});

describe("CP9 capture consent boundary", () => {
  it("does not request a sensor merely by rendering an open project", () => {
    const getDisplayMedia = vi.fn();
    const getUserMedia = vi.fn();
    Object.defineProperty(navigator, "mediaDevices", {
      configurable: true,
      value: { getDisplayMedia, getUserMedia },
    });
    render(<CapturePanel projectOpen researchEnabled />);
    expect(getDisplayMedia).not.toHaveBeenCalled();
    expect(getUserMedia).not.toHaveBeenCalled();
    expect(screen.getByRole("button", { name: "Start capture" })).toBeDisabled();
  });
  it("starts native screen capture only on a consented click and saves to Workspace",async()=>{
    const getDisplayMedia=vi.fn();
    Object.defineProperty(navigator,"mediaDevices",{configurable:true,value:{getDisplayMedia,getUserMedia:vi.fn()}});
    const busy=vi.fn(),status=vi.fn();
    render(<CapturePanel projectOpen researchEnabled onRecordingChange={busy} onCaptureStatusChange={status}/>);
    await screen.findByText(/Native screen runtime is ready/);
    fireEvent.click(screen.getByLabelText("I confirm I have permission to capture this screen and audio."));
    fireEvent.click(screen.getByRole("button",{name:"Start capture"}));
    await waitFor(()=>expect(vi.mocked(invoke).mock.calls.some(call=>call[0]==="start_native_screen_capture")).toBe(true));
    expect(getDisplayMedia).not.toHaveBeenCalled();
    await waitFor(()=>expect(status).toHaveBeenLastCalledWith("Recording"));
    expect(busy).toHaveBeenLastCalledWith(true);
    expect(vi.mocked(invoke).mock.calls.some(call=>call[0]==="create_capture_session")).toBe(false);
    fireEvent.click(screen.getByRole("button",{name:"Stop"}));
    await waitFor(()=>expect(vi.mocked(invoke).mock.calls.some(call=>call[0]==="stop_native_screen_capture")).toBe(true));
    await screen.findByText(/Recording saved in Workspace/);
    expect(screen.getByRole("button",{name:"Start capture"})).toBeEnabled();
  });
  it("does not open an audio sensor if no supported recording container exists",()=>{
    const getDisplayMedia=vi.fn();
    Object.defineProperty(navigator,"mediaDevices",{configurable:true,value:{getDisplayMedia}});
    vi.stubGlobal("MediaRecorder",class {static isTypeSupported(){return false;}});
    render(<CapturePanel projectOpen researchEnabled/>);
    fireEvent.click(screen.getByLabelText("Screen"));
    fireEvent.click(screen.getByLabelText("Microphone"));
    fireEvent.click(screen.getByLabelText("I confirm I have permission to capture this screen and audio."));
    fireEvent.click(screen.getByRole("button",{name:"Start capture"}));
    expect(getDisplayMedia).not.toHaveBeenCalled();
    expect(screen.getByText(/No supported WebM recording encoder/)).toBeInTheDocument();
  });
  it.each([
    [true,false], [false,true], [true,true],
  ])("records Screen with system audio=%s and microphone=%s through the native mixer",async(systemAudio,microphone)=>{
    const getDisplayMedia=vi.fn();
    const getUserMedia=vi.fn();
    Object.defineProperty(navigator,"mediaDevices",{configurable:true,value:{getDisplayMedia,getUserMedia}});
    render(<CapturePanel projectOpen researchEnabled/>);
    await screen.findByText(/Native screen runtime is ready/);
    if(systemAudio)fireEvent.click(screen.getByLabelText("System audio"));
    if(microphone)fireEvent.click(screen.getByLabelText("Microphone"));
    fireEvent.click(screen.getByLabelText("I confirm I have permission to capture this screen and audio."));
    fireEvent.click(screen.getByRole("button",{name:"Start capture"}));
    await waitFor(()=>expect(vi.mocked(invoke)).toHaveBeenCalledWith("start_native_screen_capture",{
      systemAudio,microphone,consent:true,
    }));
    expect(getDisplayMedia).not.toHaveBeenCalled();
    expect(getUserMedia).not.toHaveBeenCalled();
  });
  it("blocks screen plus audio before requesting devices when native mixer components are absent",async()=>{
    vi.mocked(invoke).mockImplementation(async(command)=>command==="capture_runtime_health"?{
      platform:"linux",
      native_screen_available:true,native_screen_audio_available:false,
      missing_native_screen_audio_elements:["audiomixer"],summary:"Audio mixer missing",
    }:{items:[],next_offset:null});
    const getDisplayMedia=vi.fn();
    Object.defineProperty(navigator,"mediaDevices",{configurable:true,value:{getDisplayMedia}});
    render(<CapturePanel projectOpen researchEnabled/>);
    await screen.findByText(/Audio mixer missing/);
    fireEvent.click(screen.getByLabelText("Microphone"));
    fireEvent.click(screen.getByLabelText("I confirm I have permission to capture this screen and audio."));
    fireEvent.click(screen.getByRole("button",{name:"Start capture"}));
    expect(screen.getByText(/Screen audio recording is unavailable: audiomixer/)).toBeInTheDocument();
    expect(getDisplayMedia).not.toHaveBeenCalled();
    expect(vi.mocked(invoke).mock.calls.some(call=>call[0]==="start_native_screen_capture")).toBe(false);
  });
  it("restores Start rather than offering an impossible retry after native finalization fails",async()=>{
    vi.mocked(invoke).mockImplementation(async(command)=>{
      if(command==="capture_runtime_health")return {platform:"linux",native_screen_available:true,native_screen_audio_available:true,summary:"Ready"};
      if(command==="stop_native_screen_capture")throw new Error("Recording is 0 bytes; nothing was added to the workspace.");
      return {items:[],next_offset:null};
    });
    render(<CapturePanel projectOpen researchEnabled/>);
    await screen.findByText(/Ready Device access is confirmed/);
    fireEvent.click(screen.getByLabelText("Microphone"));
    fireEvent.click(screen.getByLabelText("I confirm I have permission to capture this screen and audio."));
    fireEvent.click(screen.getByRole("button",{name:"Start capture"}));
    await screen.findByRole("button",{name:"Stop"});
    fireEvent.click(screen.getByRole("button",{name:"Stop"}));
    await screen.findByText(/Recording is 0 bytes/);
    expect(screen.queryByRole("button",{name:/Retry save/})).not.toBeInTheDocument();
    expect(screen.getByRole("button",{name:"Start capture"})).toBeEnabled();
  });
  it.each([
    [false,false], [true,false], [false,true], [true,true],
  ])("routes Windows Screen with system audio=%s and microphone=%s to WebView2",async(systemAudio,microphone)=>{
    vi.mocked(invoke).mockImplementation(async(command)=>command==="capture_runtime_health"?{
      platform:"windows",native_screen_available:false,native_screen_audio_available:false,
      native_system_audio_available:false,summary:"Windows WebView2 capture is ready.",
    }:{items:[],next_offset:null});
    const getDisplayMedia=vi.fn().mockRejectedValue(new DOMException("User cancelled sharing", "NotAllowedError"));
    const getUserMedia=vi.fn();
    Object.defineProperty(navigator,"mediaDevices",{configurable:true,value:{getDisplayMedia,getUserMedia}});
    vi.stubGlobal("MediaRecorder",class {static isTypeSupported(){return true;}});
    vi.stubGlobal("AudioContext",class {resume(){return Promise.resolve();}close(){return Promise.resolve();}});
    render(<CapturePanel projectOpen researchEnabled/>);
    await screen.findByText(/Windows WebView2 capture is ready/);
    if(systemAudio)fireEvent.click(screen.getByLabelText("System audio"));
    if(microphone)fireEvent.click(screen.getByLabelText("Microphone"));
    fireEvent.click(screen.getByLabelText("I confirm I have permission to capture this screen and audio."));
    fireEvent.click(screen.getByRole("button",{name:"Start capture"}));
    await waitFor(()=>expect(getDisplayMedia).toHaveBeenCalledWith({video:true,audio:systemAudio}));
    expect(vi.mocked(invoke).mock.calls.some(call=>call[0]==="start_native_screen_capture")).toBe(false);
    expect(getUserMedia).not.toHaveBeenCalled();
  });
  it.each([[false,false],[false,true],[true,false],[true,true]])(
    "uses WASAPI for Windows system audio with screen=%s microphone=%s and releases partial grants on failure",
    async(withScreen,withMicrophone)=>{
      vi.mocked(invoke).mockImplementation(async(command)=>command==="capture_runtime_health"?{
        platform:"windows",native_screen_available:false,native_system_audio_available:true,
        summary:"Windows WASAPI is ready.",
      }:{items:[],next_offset:null});
      vi.mocked(desktopAudio).mockRejectedValue(new Error("WASAPI output device disconnected"));
      const video={kind:"video",stop:vi.fn()};
      const voice={kind:"audio",stop:vi.fn()};
      const stream=(tracks:typeof video[])=>({getTracks:()=>tracks,getVideoTracks:()=>tracks.filter(t=>t.kind==="video"),getAudioTracks:()=>tracks.filter(t=>t.kind==="audio")}) as unknown as MediaStream;
      const getDisplayMedia=vi.fn().mockResolvedValue(stream([video]));
      const getUserMedia=vi.fn().mockResolvedValue(stream([voice]));
      Object.defineProperty(navigator,"mediaDevices",{configurable:true,value:{getDisplayMedia,getUserMedia}});
      vi.stubGlobal("MediaRecorder",class {static isTypeSupported(){return true;}});
      vi.stubGlobal("AudioContext",class {resume(){return Promise.resolve();}close(){return Promise.resolve();}});
      render(<CapturePanel projectOpen researchEnabled projectId="project"/>);
      await screen.findByText(/Windows WASAPI is ready/);
      if(!withScreen)fireEvent.click(screen.getByLabelText("Screen"));
      fireEvent.click(screen.getByLabelText("System audio"));
      if(withMicrophone)fireEvent.click(screen.getByLabelText("Microphone"));
      fireEvent.click(screen.getByLabelText("I confirm I have permission to capture this screen and audio."));
      fireEvent.click(screen.getByRole("button",{name:"Start capture"}));
      await screen.findByText(/WASAPI output device disconnected/);
      expect(desktopAudio).toHaveBeenCalled();
      if(withScreen){expect(getDisplayMedia).toHaveBeenCalledWith({video:true,audio:false});expect(video.stop).toHaveBeenCalled();}
      else expect(getDisplayMedia).not.toHaveBeenCalled();
      if(withMicrophone){expect(getUserMedia).toHaveBeenCalledWith(microphoneConstraints());expect(voice.stop).toHaveBeenCalled();}
      else expect(getUserMedia).not.toHaveBeenCalled();
      expect(vi.mocked(invoke).mock.calls.some(call=>call[0]==="create_capture_session")).toBe(false);
    });
});

describe("Microphone input check",()=>{
 it("requires consent, uses the chosen device and volume, and saves no capture",async()=>{
  vi.mocked(invoke).mockImplementation(async(command)=>command==="capture_runtime_health"?{platform:"windows",native_screen_available:false,summary:"Ready"}:{items:[],next_offset:null});
  const stop=vi.fn(),release=vi.fn(),setGain=vi.fn();
  const input={getTracks:()=>[{stop}],getAudioTracks:()=>[{stop}]} as unknown as MediaStream;
  const getUserMedia=vi.fn().mockResolvedValue(input);
  Object.defineProperty(navigator,"mediaDevices",{configurable:true,value:{getUserMedia,enumerateDevices:vi.fn().mockResolvedValue([
   {kind:"audioinput",deviceId:"usb-mic",label:"USB microphone"},
   {kind:"audiooutput",deviceId:"speaker",label:"Speaker"},
  ])}});
  vi.stubGlobal("AudioContext",class {resume(){return Promise.resolve();}close(){return Promise.resolve();}});
  vi.mocked(microphoneAudio).mockImplementation((_stream,_context,_gain,onLevel)=>{
   onLevel({label:"USB microphone",level:.7,received:true,muted:false,silent:false,suspended:false});
   return {stream:input,release,setGain};
  });
  const view=render(<CapturePanel projectOpen researchEnabled/>);
  fireEvent.click(screen.getByLabelText("Microphone"));
  const test=screen.getByRole("button",{name:"Test microphone"});expect(test).toBeDisabled();
  await screen.findByRole("option",{name:"USB microphone"});
  fireEvent.change(screen.getByLabelText("Microphone input"),{target:{value:"usb-mic"}});
  fireEvent.click(screen.getByLabelText("I confirm I have permission to capture this screen and audio."));
  fireEvent.click(test);
  await screen.findByText("Microphone signal received.");
  expect(getUserMedia).toHaveBeenCalledWith(microphoneConstraints("usb-mic"));
  fireEvent.change(screen.getByRole("slider",{name:/Microphone volume/}),{target:{value:"200"}});
  expect(setGain).toHaveBeenCalledWith(2);
  expect(screen.getByRole("button",{name:"Start capture"})).toBeDisabled();
  fireEvent.click(screen.getByRole("button",{name:"Stop microphone test"}));
  await screen.findByText("Microphone test finished. No recording was saved.");
  expect(stop).toHaveBeenCalledOnce();expect(release).toHaveBeenCalledOnce();
  expect(vi.mocked(invoke).mock.calls.some(call=>call[0]==="create_capture_session")).toBe(false);
  view.unmount();
 });
});
