import {describe,it,expect} from "vitest";
import {emptyBoard,changeBoard,undoBoard,redoBoard,visibleLinks,type BoardLink} from "./boardState";
describe("Reversible board edits",()=>{
  it("groups an AI batch into one undo and restores it on redo",()=>{
    const next=changeBoard(emptyBoard,{...emptyBoard.present,notes:{a:{title:"Price",description:"Unverified interpretation"}},links:[{id:"edge",source:"a",target:"b",label:"Supports",origin:"ai"}]});
    expect(next.past).toHaveLength(1);expect(undoBoard(next).present).toEqual(emptyBoard.present);expect(redoBoard(undoBoard(next)).present).toEqual(next.present);
  });
  it("keeps removed canonical links hidden after rerender and restores them on undo",()=>{
    const link:BoardLink={id:"edge",source:"a",target:"b",label:"supports",origin:"project"};const ids=new Set(["a","b"]);
    const removed=changeBoard(emptyBoard,{...emptyBoard.present,hiddenEdges:["edge"]});
    expect(visibleLinks(removed.present,[link],ids)).toEqual([]);expect(visibleLinks(undoBoard(removed).present,[link],ids)).toEqual([link]);
  });
  it("new changes discard redo, bound history, and hide dangling edges",()=>{
    let state=emptyBoard;for(let i=0;i<55;i++)state=changeBoard(state,{...state.present,positions:{a:{x:i,y:0}}});expect(state.past).toHaveLength(40);
    const changed=changeBoard(undoBoard(state),{...state.present,groups:{a:"Pricing"}});expect(changed.future).toEqual([]);
    expect(visibleLinks({...emptyBoard.present,links:[{id:"x",source:"a",target:"missing",label:"supports",origin:"user"}]},[],new Set(["a"]))).toEqual([]);
  });
});
