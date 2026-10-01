import {fireEvent,render,screen,waitFor,cleanup} from "@testing-library/react";
import {afterEach,beforeEach,expect,it,vi} from "vitest";
import {invoke} from "@tauri-apps/api/core";
import {EvidenceMedia} from "./EvidenceMedia";
import type {WorkspaceItem} from "./workspaceBridge";

vi.mock("@tauri-apps/api/core",()=>({invoke:vi.fn()}));

const recording:WorkspaceItem={
  entity:{id:"evidence-video",title:"Testing Screen Record",entity_type:"evidence",status:"available",version:1,updated_at:"2026-09-29",metadata:{}},
  details:{kind:"recording",media_type:"video/webm; codecs=vp8",original_artifact_id:"artifact-video"},
};

beforeEach(()=>{
  vi.mocked(invoke).mockResolvedValue({artifact_id:"artifact-video",media_type:"video/webm; codecs=vp8",bytes:[1,2,3]});
  URL.createObjectURL=vi.fn().mockReturnValue("blob:recording");
  URL.revokeObjectURL=vi.fn();
});
afterEach(()=>{cleanup();vi.clearAllMocks();});

it("loads a saved recording directly in the evidence dialog with playback controls",async()=>{
  const {container}=render(<EvidenceMedia item={recording} projectId="project"/>);
  await waitFor(()=>expect(container.querySelector("video[controls]")).toBeTruthy());
  expect(container.querySelector("video")?.getAttribute("src")).toBe("blob:recording");
  expect(invoke).toHaveBeenCalledWith("workspace_media",{projectId:"project",artifactId:"artifact-video"});
  expect(screen.queryByText(/Preview unavailable/)).not.toBeInTheDocument();
});

it("waits for an explicit click before loading a recording on the board",async()=>{
  const {container}=render(<EvidenceMedia item={recording} projectId="project" thumbnail/>);
  expect(invoke).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("button",{name:"▶ Open recording"}));
  await waitFor(()=>expect(container.querySelector("video[controls]")).toBeTruthy());
});
