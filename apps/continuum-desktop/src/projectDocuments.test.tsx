import {renderHook,act,waitFor} from "@testing-library/react";
import {beforeEach,describe,it,expect,vi} from "vitest";
import {useProjectDocument,flushProjectDocuments} from "./projectDocuments";
const api=vi.hoisted(()=>({invoke:vi.fn()}));vi.mock("@tauri-apps/api/core",()=>api);
beforeEach(()=>{api.invoke.mockReset();});
describe("Durable document queue",()=>{
 it("flushes a delayed edit exactly once before closing",async()=>{
  api.invoke.mockImplementation(async(command,args)=>command==="load_workspace_document"?null:{key:args.key,payload:args.payload,revision:args.expectedRevision+1});
  const {result}=renderHook(()=>useProjectDocument("p","report:test","report",{markdown:""}));await waitFor(()=>expect(result.current.ready).toBe(true));
  act(()=>result.current.save({markdown:"latest edit"},true));await act(()=>flushProjectDocuments());
  expect(api.invoke.mock.calls.filter(c=>c[0]==="save_workspace_document")).toHaveLength(1);expect(result.current.revision).toBe(1);
 });
 it("serializes revision numbers and preserves a conflicted draft",async()=>{
  api.invoke.mockImplementation(async(command,args)=>command==="load_workspace_document"?null:{key:args.key,payload:args.payload,revision:args.expectedRevision+1});
  const {result}=renderHook(()=>useProjectDocument("q","board:test","board",{text:""}));await waitFor(()=>expect(result.current.ready).toBe(true));
  act(()=>{result.current.save({text:"first"});result.current.save({text:"second"});});await act(()=>flushProjectDocuments());
  expect(api.invoke.mock.calls.filter(c=>c[0]==="save_workspace_document").map(c=>c[1].expectedRevision)).toEqual([0,1]);
  api.invoke.mockRejectedValueOnce(new Error("Revision conflict"));act(()=>result.current.save({text:"unsaved third"}));
  await expect(flushProjectDocuments()).rejects.toThrow("Revision conflict");expect(result.current.ref.current.text).toBe("unsaved third");
  api.invoke.mockResolvedValue({revision:2,payload:{text:"second"}});await act(()=>result.current.reload());
 });
});
