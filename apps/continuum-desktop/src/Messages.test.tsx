import {fireEvent, render, screen, waitFor} from "@testing-library/react";
import {describe, expect, it, vi} from "vitest";
import {Messages} from "./Messages";

const api=vi.hoisted(()=>({save:vi.fn(),run:vi.fn()}));
vi.mock("./projectDocuments",()=>({
  useProjectDocument:()=>({value:{messages:[]},ref:{current:{messages:[]}},save:api.save,ready:true,error:""}),
  runAssistant:api.run,
}));
vi.mock("./MarkdownReading",()=>({MarkdownReading:({markdown}:{markdown:string})=><p>{markdown}</p>}));

describe("Messages",()=>{
  it("keeps a failed question editable and does not save duplicate user turns",async()=>{
    api.save.mockClear();api.run.mockReset().mockRejectedValue(new Error("temporary assistant failure"));
    render(<Messages projectId="project" scope="research" sessionId={null} agent="codex"/>);
    fireEvent.change(screen.getByLabelText("Message"),{target:{value:"What changed?"}});
    fireEvent.click(screen.getByRole("button",{name:"Send"}));
    await waitFor(()=>expect(screen.getByRole("alert")).toHaveTextContent("temporary assistant failure"));
    expect(api.save).not.toHaveBeenCalled();
    expect(screen.getByLabelText("Message")).toHaveValue("What changed?");
  });
});
