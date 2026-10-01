import {render,screen,fireEvent,waitFor} from "@testing-library/react";
import {beforeEach,it,expect,vi} from "vitest";
import {ReportStudio} from "./ReportStudio";
import type {HumanDocument} from "./contracts";
const api=vi.hoisted(()=>({invoke:vi.fn()}));
vi.mock("@tauri-apps/api/core",()=>api);
vi.mock("./MarkdownReading",()=>({MarkdownReading:({markdown}:{markdown:string})=><article>{markdown}</article>}));
vi.mock("./HumanDocumentView",()=>({HumanDocumentView:()=>null}));
vi.mock("./MermaidDiagram",()=>({mermaidSource:()=>""}));
vi.mock("./workspaceBridge",()=>({copyText:vi.fn()}));
const document={title:"Old template title",blocks:[],citations:[],source_ledger_sequence:10} as unknown as HumanDocument;
beforeEach(()=>{api.invoke.mockReset();api.invoke.mockImplementation(async(command,args)=>{
 if(command==="load_workspace_document")return null;
 if(command==="recent_workspace_ai"||command==="list_mcp_report_drafts")return [];
 if(command==="save_workspace_document")return {revision:args.expectedRevision+1,payload:args.payload};
 throw new Error(command);
});});
it("starts empty, does not seed legacy sections or spend tokens on mount",async()=>{
 render(<ReportStudio projectId="p" scope="research" sessionId="s" document={document} agent="codex"/>);
 await waitFor(()=>expect(screen.getByRole("button",{name:"Write Markdown"})).toBeEnabled());
 expect(screen.getByText("No report yet")).toBeInTheDocument();
 expect(screen.queryByText("Old template title")).not.toBeInTheDocument();
 expect(screen.getByRole("button",{name:"Export HTML"})).toBeDisabled();
 expect(api.invoke.mock.calls.some(c=>["save_workspace_document","run_workspace_assistant"].includes(c[0]))).toBe(false);
});
it("previews MCP Markdown verbatim and only saves it after explicit adoption",async()=>{
 const base=api.invoke.getMockImplementation()!;
 api.invoke.mockImplementation((command,args)=>command==="list_mcp_report_drafts"?Promise.resolve([{markdown:"# Custom findings\nEvidence explained in my own structure.",sources:[],sourceSequence:10,status:"draft",origin:"mcp",proposalId:"proposal1",title:"MCP research"}]):base(command,args));
 render(<ReportStudio projectId="q" scope="research" sessionId="s" document={document} agent="codex"/>);
 fireEvent.click(screen.getByText(/Use a connected AI through MCP/));
 await waitFor(()=>expect(screen.getByRole("button",{name:"Preview draft"})).toBeEnabled());
 fireEvent.click(screen.getByRole("button",{name:"Preview draft"}));
 expect(api.invoke.mock.calls.some(c=>c[0]==="save_workspace_document")).toBe(false);
 fireEvent.click(screen.getByRole("button",{name:"Use this draft"}));
 await waitFor(()=>expect(api.invoke.mock.calls.find(c=>c[0]==="save_workspace_document")?.[1].payload.markdown).toBe("# Custom findings\nEvidence explained in my own structure."));
 expect(screen.getByText("Draft received through MCP")).toBeInTheDocument();
});
