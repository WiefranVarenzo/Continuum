import {fireEvent,render,screen,waitFor} from "@testing-library/react";
import {it,expect,vi} from "vitest";
import {ResearchBoard} from "./ResearchBoard";
const windowApi=vi.hoisted(()=>({isFullscreen:vi.fn().mockResolvedValue(false),setFullscreen:vi.fn().mockResolvedValue(undefined)}));
vi.mock("@tauri-apps/api/window",()=>({getCurrentWindow:()=>windowApi}));
vi.mock("@tauri-apps/api/core",()=>({invoke:vi.fn().mockResolvedValue(null)}));
vi.mock("./EvidenceMedia",()=>({EvidenceMedia:()=>null}));
it("mounts the real React Flow board without recursive selection updates",async()=>{
 vi.stubGlobal("ResizeObserver",class {observe(){}unobserve(){}disconnect(){}});
 const item={entity:{id:"a",title:"Evidence A",entity_type:"evidence",status:"available",version:1,updated_at:"today",metadata:{}},details:{annotation:"An observation"}};
 const view=render(<ResearchBoard items={[item]} session={null} onInspect={()=>{}} onAnalyze={()=>{}} analyzing={false} revision={0} projectId="test" boardKey="test"/>);
 await waitFor(()=>expect(screen.getByText("Evidence A")).toBeInTheDocument());expect(screen.getByRole("button",{name:"Undo"})).toBeDisabled();view.unmount();vi.unstubAllGlobals();
});
it("expands the workspace board and restores the window on Escape",async()=>{
 vi.stubGlobal("ResizeObserver",class {observe(){}unobserve(){}disconnect(){}});
 const view=render(<ResearchBoard items={[]} session={null} onInspect={()=>{}} onAnalyze={()=>{}} analyzing={false} revision={0} projectId="test" boardKey="fullscreen"/>);
 await waitFor(()=>expect(screen.getByRole("button",{name:"Full screen"})).toBeEnabled());
 fireEvent.click(screen.getByRole("button",{name:"Full screen"}));
 await waitFor(()=>expect(windowApi.setFullscreen).toHaveBeenCalledWith(true));
 expect(await screen.findByRole("button",{name:"Exit full screen"})).toHaveAttribute("aria-pressed","true");
 fireEvent.keyDown(window,{key:"Escape"});
 await waitFor(()=>expect(windowApi.setFullscreen).toHaveBeenCalledWith(false));
 expect(await screen.findByRole("button",{name:"Full screen"})).toHaveAttribute("aria-pressed","false");
 view.unmount();vi.unstubAllGlobals();
});
