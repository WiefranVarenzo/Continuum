import { act, fireEvent, render, screen, waitFor, cleanup } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { Workspace } from "./Workspace";
import { sampleDocument } from "./sample";
import type { WorkspaceItem } from "./workspaceBridge";
const windowApi=vi.hoisted(()=>({onCloseRequested:vi.fn(),destroy:vi.fn()}));
vi.mock("@tauri-apps/api/window",()=>({getCurrentWindow:()=>windowApi}));
vi.mock("@tauri-apps/api/core",()=>({invoke:vi.fn().mockResolvedValue(null)}));
vi.mock("./Messages",()=>({Messages:()=> <div>Messages content</div>}));
vi.mock("./MemoryPanel",()=>({MemoryPanel:()=> <div>Bookmark content</div>}));
vi.mock("./ReportStudio",()=>({ReportStudio:()=> <div>Report content</div>}));
vi.mock("./projectDocuments",()=>({runAssistant:(...args:unknown[])=>api.organize(...args),flushProjectDocuments:vi.fn().mockResolvedValue(undefined),registerCloseTask:()=>()=>{},prepareWorkspaceClose:api.prepareClose}));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn().mockResolvedValue(() => undefined) }));

const api = vi.hoisted(() => ({ researchItems: vi.fn(), startResearch: vi.fn(), createCheckpoint: vi.fn(), capture: vi.fn(), archive: vi.fn(), clipboard: vi.fn(), annotate: vi.fn(), updateEvidence: vi.fn(), organize: vi.fn(), prepareClose:vi.fn() }));
vi.mock("./workspaceBridge", () => ({ researchItems: api.researchItems, startResearch: api.startResearch, archiveEvidence: api.archive, annotateEvidence: api.annotate, updateEvidence: api.updateEvidence, updateResearchSession: vi.fn(), updateResearchQuestion: vi.fn(), clipboardPng: api.clipboard, imageFingerprint: vi.fn().mockResolvedValue("fingerprint"), detectClients: vi.fn().mockResolvedValue([]), runAiOrganizer: api.organize, exportContextMarkdown: vi.fn().mockResolvedValue("/project/context.md"), setCaptureDock: vi.fn().mockResolvedValue(undefined) }));
vi.mock("./bridge", () => ({ createCheckpoint: api.createCheckpoint, captureExternalEvidence: api.capture, getCurrentProjectState: vi.fn().mockResolvedValue({as_of_ledger_sequence:1}) }));
vi.mock("./ResearchBoard", () => ({ ResearchBoard: () => <div>Research board</div>, EvidenceImage: () => <div>Image preview</div> }));
vi.mock("./KnowledgeGraph", () => ({ KnowledgeGraph: () => <div>Graph</div> }));
vi.mock("./CapturePanel", () => ({ CapturePanel: () => <div>Recording controls</div> }));
vi.mock("./AiContinuityPanel", () => ({ AiContinuityPanel: () => <div>AI access</div> }));
vi.mock("./ContinuityPanel", () => ({ ContinuityPanel: () => <div>Memory controls</div> }));
vi.mock("./HumanDocumentView", () => ({ HumanDocumentView: () => <div>Report content</div> }));
vi.mock("./WorkspaceQuickStart", () => ({ WorkspaceQuickStart: () => <div>Repository controls</div> }));
const session: WorkspaceItem = { entity: {id:"session-one",title:"Research one",entity_type:"research_session",status:"active",version:1,updated_at:"2026-09-15",metadata:{}},details:{objective:"Learn about continuity"} };
const evidence: WorkspaceItem = { entity: {id:"evidence-one",title:"Saved screenshot",entity_type:"evidence",status:"available",version:1,updated_at:"2026-09-15",metadata:{}},details:{annotation:"",summary:"Observation"} };
const project = {project_id:"p",name:"Project",path:"/project",status:"active",ledger_sequence:1,research:true,development:false,integrity_healthy:true};
const props = {project,capabilities:{research:true,development:false},document:sampleDocument,settings:<div>Settings</div>,busy:false,onRefresh:vi.fn().mockResolvedValue(undefined),onExport:vi.fn(),onRepository:vi.fn(),onAllProjects:vi.fn().mockResolvedValue(undefined)};
beforeEach(() => { localStorage.clear(); vi.clearAllMocks(); windowApi.onCloseRequested.mockResolvedValue(()=>{});windowApi.destroy.mockResolvedValue(undefined);api.prepareClose.mockResolvedValue(undefined); vi.spyOn(window,"confirm").mockReturnValue(true); api.researchItems.mockResolvedValue({items:[],next_offset:null}); api.createCheckpoint.mockResolvedValue({}); api.startResearch.mockResolvedValue(session); api.capture.mockResolvedValue(evidence); api.updateEvidence.mockResolvedValue(evidence); api.clipboard.mockResolvedValue(null); api.organize.mockResolvedValue({client_name:"Codex",evidence_count:1,summary:"3 proposals submitted."}); });
afterEach(() => { cleanup(); vi.restoreAllMocks(); });

describe("Activity-based workspace", () => {
  it("saves before closing and destroys the window without a second close request",async()=>{
    render(<Workspace {...props}/>);
    await waitFor(()=>expect(windowApi.onCloseRequested).toHaveBeenCalled());
    const preventDefault=vi.fn();
    await act(async()=>{windowApi.onCloseRequested.mock.calls[0][0]({preventDefault});});
    expect(preventDefault).toHaveBeenCalled();
    await waitFor(()=>expect(api.prepareClose).toHaveBeenCalledOnce());
    expect(windowApi.destroy).toHaveBeenCalledOnce();
  });
  it("shows an actionable dialog when saving blocks exit",async()=>{
    api.prepareClose.mockRejectedValueOnce(new Error("Stop the live capture before closing."));
    render(<Workspace {...props}/>);
    await waitFor(()=>expect(windowApi.onCloseRequested).toHaveBeenCalled());
    await act(async()=>{windowApi.onCloseRequested.mock.calls[0][0]({preventDefault:vi.fn()});});
    expect(await screen.findByRole("dialog",{name:"Continuum could not close yet"})).toHaveTextContent("Stop the live capture before closing.");
    expect(windowApi.destroy).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button",{name:"Keep working"}));
    expect(screen.queryByRole("dialog",{name:"Continuum could not close yet"})).not.toBeInTheDocument();
  });
  it("starts a research with context, hides onboarding, and saves its resume point automatically", async () => {
    render(<Workspace {...props} />);
    fireEvent.click(screen.getByRole("button",{name:"Workspace"}));
    await screen.findByRole("heading",{name:"What are you researching?"});
    fireEvent.change(screen.getByLabelText("Short title"),{target:{value:"Research one"}});
    fireEvent.change(screen.getByLabelText("Question"),{target:{value:"Learn about continuity"}});
    fireEvent.click(screen.getByRole("button",{name:"Start research →"}));
    await waitFor(() => expect(api.startResearch).toHaveBeenCalledWith(expect.objectContaining({title:"Research one",question:"Learn about continuity"})));
    await waitFor(() => expect(screen.queryByRole("heading",{name:"What are you researching?"})).not.toBeInTheDocument());
    expect(localStorage.getItem("continuum.workspace.p.session")).toBe("session-one");
    await waitFor(() => expect(api.createCheckpoint).toHaveBeenCalledWith(expect.objectContaining({trigger:"policy",scope:"research"})),{timeout:3500});
  });
  it("restores the selected research and gates capture without reading clipboard on startup", async () => {
    localStorage.setItem("continuum.workspace.p.session","session-one");
    api.researchItems.mockImplementation((_session, onlySessions) => Promise.resolve({items:onlySessions?[session]:[evidence],next_offset:null}));
    render(<Workspace {...props} />);
    fireEvent.click(screen.getByRole("button",{name:"Workspace"}));
    await screen.findByRole("button",{name:"Live screenshots"});
    expect(api.clipboard).not.toHaveBeenCalled();
    expect(screen.getByRole("button",{name:"Recent captures (1)"})).toBeVisible();
    expect(screen.getByText("Recording controls")).not.toBeVisible();
    fireEvent.click(screen.getByRole("button",{name:"Record & sources"}));
    expect(screen.getByText("Recording controls")).toBeVisible();
    expect(screen.queryByText("Report content")).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole("button",{name:"Reports"}));
    expect(screen.getByText("Report content")).toBeVisible();
  });
  it("archives the selected screenshot and leaves other sources untouched", async () => {
    localStorage.setItem("continuum.workspace.p.session","session-one");
    api.researchItems.mockImplementation((_session, onlySessions) => Promise.resolve({items:onlySessions?[session]:[evidence],next_offset:null}));
    api.archive.mockResolvedValue({...evidence,entity:{...evidence.entity,status:"archived"}});
    render(<Workspace {...props} />);
    fireEvent.click(screen.getByRole("button",{name:"Workspace"}));
    fireEvent.click(await screen.findByRole("button",{name:"Remove Saved screenshot"}));
    await waitFor(() => expect(api.archive).toHaveBeenCalledWith(evidence));
    expect(await screen.findByText(/original remains in project history/)).toBeVisible();
  });
  it("runs the selected assistant and labels board output as draft suggestions", async () => {
    api.organize.mockResolvedValue({job_id:"job1",output:{markdown:"Analysis",notes:[],connections:[{source:"a",target:"b",label:"Supports",reason:"Evidence"}]},image_count:1});
    localStorage.setItem("continuum.workspace.p.session","session-one");
    api.researchItems.mockImplementation((_session, onlySessions) => Promise.resolve({items:onlySessions?[session]:[evidence],next_offset:null}));
    render(<Workspace {...props} />);
    fireEvent.click(screen.getByRole("button",{name:"Workspace"}));
    fireEvent.click(await screen.findByRole("button",{name:"Analyze & connect with AI"}));
    await waitFor(() => expect(api.organize).toHaveBeenCalledWith("p","research","session-one","codex","board",expect.any(String)));
    expect(await screen.findByText(/Analysis completed: 0 draft descriptions, 1 suggested connections/)).toBeVisible();
  });
  it("does not show research-only entry actions in a development workspace", async () => {
    render(<Workspace {...props} project={{...project,research:false,development:true}} capabilities={{research:false,development:true}} />);
    await waitFor(() => expect(screen.queryByText("Restoring your work…")).not.toBeInTheDocument());
    expect(screen.queryByRole("button",{name:"＋ New research"})).not.toBeInTheDocument();
    expect(screen.getByText("Development only",{selector:".workspace-mode"})).toBeVisible();
    fireEvent.click(screen.getByRole("button",{name:"Development"}));
    expect(screen.getByText("Repository controls")).toBeVisible();
  });
});
