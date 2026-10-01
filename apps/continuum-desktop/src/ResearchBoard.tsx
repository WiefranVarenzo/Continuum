import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { Background, Controls, ReactFlow, Position, Handle, MiniMap, MarkerType, useNodesState, type Node, type NodeProps, type Connection } from "@xyflow/react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import ELK from "elkjs/lib/elk.bundled.js";
import type { GraphSpecification } from "./contracts";
import type { WorkspaceItem } from "./workspaceBridge";
import { EvidenceMedia } from "./EvidenceMedia";
import { useProjectDocument, type AssistantResult } from "./projectDocuments";
import {emptyBoard,changeBoard,undoBoard,redoBoard,visibleLinks,type BoardSnapshot,type BoardLink} from "./boardState";
import "@xyflow/react/dist/style.css";

export function EvidenceImage({item,projectId="",thumbnail=true}:{item:WorkspaceItem;projectId?:string;thumbnail?:boolean}) {return <EvidenceMedia item={item} projectId={projectId} thumbnail={thumbnail}/>;}
type CardNode=Node<{item:WorkspaceItem;projectId:string;group?:string;note?:{title:string;description:string};inspect:()=>void}>;
function ResearchCard({data}:NodeProps<CardNode>){
  const {item}=data;
  const raw=String(item.details.annotation||item.details.claim||item.details.question||item.details.objective||item.details.summary||"");
  const description=/Captured screenshot;|Awaiting AI analysis/.test(raw)?"Add context or ask AI to explore this source.":raw;
  return <><Handle type="target" position={Position.Left}/><div className="board-card-content">
    {data.group&&<div className="board-group">{data.group}</div>}
    <EvidenceMedia item={item} projectId={data.projectId} thumbnail/>
    <small className="board-kind">{item.entity.entity_type.replaceAll("_"," ")}</small>
    <strong>{item.entity.title}</strong><p>{description.slice(0,220)}</p>
    {data.note&&<details className="nodrag"><summary>AI interpretation · draft</summary><strong>{data.note.title}</strong><p>{data.note.description}</p></details>}
    <button className="nodrag board-card-edit" onClick={data.inspect}>Open / edit</button>
  </div><Handle type="source" position={Position.Right}/></>;
}
const nodeTypes={researchCard:ResearchCard};
const elk=new ELK();
export function ResearchBoard({items,session,graph,onInspect,onAnalyze,analyzing,projectId,boardKey,aiDraft}: {
  items:WorkspaceItem[];session:WorkspaceItem|null;graph?:GraphSpecification;onInspect:(item:WorkspaceItem)=>void;
  onAnalyze:()=>void;analyzing:boolean;revision:number;projectId:string;boardKey:string;aiDraft?:AssistantResult|null;
}){
  const doc=useProjectDocument(projectId,"board:"+boardKey,"board",emptyBoard);
  const [nodes,setNodes,onNodesChange]=useNodesState<CardNode>([]);
  const [selected,setSelected]=useState<string[]>([]),[edgeId,setEdgeId]=useState<string|null>(null);
  const [label,setLabel]=useState(""),[reason,setReason]=useState(""),[group,setGroup]=useState("");
  const [layoutBusy,setLayoutBusy]=useState(false),[failure,setFailure]=useState("");
  const [fullScreen,setFullScreen]=useState(false),[fullScreenBusy,setFullScreenBusy]=useState(false);
  const restoreWindowFullscreen=useRef(false);
  const state=doc.value.present;
  const all=useMemo(()=>[...new Map([...(session?[session]:[]),...items.filter(i=>i.entity.status!=="archived")].map(i=>[i.entity.id,i])).values()],[items,session]);
  const records=useMemo(()=>all.filter(i=>!state.hiddenNodes.includes(i.entity.id)),[all,state.hiddenNodes]);
  const ids=useMemo(()=>new Set(records.map(i=>i.entity.id)),[records]);
  const projectLinks=useMemo<BoardLink[]>(()=> (graph?.edges||[]).map(e=>({...e,origin:"project"})),[graph]);
  const links=useMemo(()=>visibleLinks(state,projectLinks,ids),[state,projectLinks,ids]);
  const edges=useMemo(()=>links.map(e=>({...e,type:"bezier",markerEnd:{type:MarkerType.ArrowClosed,color:e.origin==="ai"?"#bd97e9":"#79bca5"},interactionWidth:24,selected:e.id===edgeId,style:{stroke:e.origin==="ai"?"#bd97e9":"#79bca5",strokeWidth:2,strokeDasharray:e.origin==="ai"?"6 4":undefined},labelStyle:{fill:"var(--text)",fontSize:12},labelBgStyle:{fill:"var(--panel)"}})),[links,edgeId]);
  const selectionChanged=useCallback(({nodes}:{nodes:CardNode[]})=>{const ids=nodes.map(n=>n.id);setSelected(previous=>previous.length===ids.length&&previous.every((id,index)=>id===ids[index])?previous:ids);},[]);
  const commit=(next:BoardSnapshot)=>doc.save(changeBoard(doc.ref.current,next));
  const edit=(transform:(state:BoardSnapshot)=>BoardSnapshot)=>commit(transform(doc.ref.current.present));
  useEffect(()=>{
    setNodes(records.map((item,index)=>({id:item.entity.id,type:"researchCard",position:state.positions[item.entity.id]||{x:(index%3)*370,y:Math.floor(index/3)*440},style:{width:300},selected:selected.includes(item.entity.id),data:{item,projectId,group:state.groups[item.entity.id],note:state.notes[item.entity.id],inspect:()=>onInspect(item)}})));
  },[records,state.positions,state.groups,state.notes,projectId,onInspect]);
  useEffect(()=>{
    if(!doc.ready||!aiDraft||doc.ref.current.appliedJobs.includes(aiDraft.job_id))return;
    const current=doc.ref.current;
    const notes={...current.present.notes};
    for(const note of aiDraft.output.notes)if(all.some(i=>i.entity.id===note.id))notes[note.id]=note;
    const links:BoardLink[]=aiDraft.output.connections.filter(e=>all.some(i=>i.entity.id===e.source)&&all.some(i=>i.entity.id===e.target)&&!current.present.dismissedPairs?.includes(e.source+":"+e.target)&&!current.present.links.some(link=>link.source===e.source&&link.target===e.target&&link.label===e.label)).map((e,i)=>({...e,id:"ai-"+aiDraft.job_id+"-"+i,origin:"ai"}));
    const next=changeBoard(current,{...current.present,notes,links:[...current.present.links,...links]});
    doc.save({...next,appliedJobs:[...current.appliedJobs,aiDraft.job_id].slice(-100)});
  },[aiDraft,doc.ready,all]);
  function connect(connection:Connection){if(!connection.source||!connection.target||connection.source===connection.target)return;
    const id=crypto.randomUUID();edit(s=>({...s,links:[...s.links,{id,source:connection.source!,target:connection.target!,label:"Related to",reason:"",origin:"user"}]}));setEdgeId(id);setLabel("Related to");setReason("");}
  function removeEdges(edgeIds:string[]){edit(s=>({...s,links:s.links.filter(e=>!edgeIds.includes(e.id)),hiddenEdges:[...new Set([...s.hiddenEdges,...edgeIds])],dismissedPairs:[...new Set([...(s.dismissedPairs||[]),...links.filter(e=>edgeIds.includes(e.id)).map(e=>e.source+":"+e.target)])]}));setEdgeId(null);}
  async function arrange(){
    setLayoutBusy(true);setFailure("");const before=doc.ref.current;
    try{const result=await elk.layout({id:"root",layoutOptions:{"elk.algorithm":"layered","elk.direction":"RIGHT","elk.spacing.nodeNode":"80","elk.layered.spacing.nodeNodeBetweenLayers":"180"},children:records.map(i=>({id:i.entity.id,width:300,height:nodes.find(n=>n.id===i.entity.id)?.measured?.height||(typeof i.details.original_artifact_id==="string"?390:260)})),edges:links.map(e=>({id:e.id,sources:[e.source],targets:[e.target]}))});
      if(doc.ref.current!==before){setFailure("The board changed while arranging. Click Auto layout again.");return;}
      const positions={...before.present.positions};for(const node of result.children||[])positions[node.id]={x:node.x||0,y:node.y||0};commit({...before.present,positions});
    }catch(cause){setFailure(String(cause));}finally{setLayoutBusy(false);}
  }
  async function toggleFullScreen(){
    if(fullScreenBusy)return;
    setFullScreenBusy(true);setFailure("");
    try{
      const appWindow=getCurrentWindow();
      if(fullScreen){
        if(restoreWindowFullscreen.current)await appWindow.setFullscreen(false);
        restoreWindowFullscreen.current=false;setFullScreen(false);
      }else{
        const wasFullScreen=await appWindow.isFullscreen();
        if(!wasFullScreen)await appWindow.setFullscreen(true);
        restoreWindowFullscreen.current=!wasFullScreen;setFullScreen(true);
      }
    }catch(cause){setFailure(`Could not change full-screen mode: ${String(cause)}`);}
    finally{setFullScreenBusy(false);}
  }
  useEffect(()=>{
    if(!fullScreen)return;
    const exit=(event:KeyboardEvent)=>{if(event.key==="Escape"){event.preventDefault();void toggleFullScreen();}};
    window.addEventListener("keydown",exit);
    return()=>window.removeEventListener("keydown",exit);
  },[fullScreen,fullScreenBusy]);
  useEffect(()=>()=>{if(restoreWindowFullscreen.current)void getCurrentWindow().setFullscreen(false).catch(()=>undefined);},[]);
  return <div className={`research-board ${fullScreen?"is-fullscreen":""}`} tabIndex={0} onKeyDown={event=>{
    if((event.target as HTMLElement).closest("input,textarea,select,[contenteditable=true]"))return;
    if((event.ctrlKey||event.metaKey)&&event.key.toLowerCase()==="z"){event.preventDefault();doc.save(event.shiftKey?redoBoard(doc.ref.current):undoBoard(doc.ref.current));}
    if((event.ctrlKey||event.metaKey)&&event.key.toLowerCase()==="y"){event.preventDefault();doc.save(redoBoard(doc.ref.current));}
  }}>
    <div className="board-caption"><span>{records.length} cards · {links.length} connections · {doc.saving?"Saving…":"Saved locally"}</span><div>
      <button disabled={!doc.ready||!doc.value.past.length||!!doc.error} onClick={()=>doc.save(undoBoard(doc.ref.current))}>Undo</button>
      <button disabled={!doc.ready||!doc.value.future.length||!!doc.error} onClick={()=>doc.save(redoBoard(doc.ref.current))}>Redo</button>
      <button disabled={!doc.ready||analyzing||!records.length} onClick={onAnalyze}>{analyzing?"AI is analyzing…":"Analyze & connect with AI"}</button>
      <button disabled={!doc.ready||layoutBusy} onClick={()=>void arrange()}>{layoutBusy?"Arranging…":"Auto layout"}</button>
      <button type="button" disabled={fullScreenBusy} aria-pressed={fullScreen} onClick={()=>void toggleFullScreen()}>{fullScreen?"Exit full screen":"Full screen"}</button>
    </div></div>
    {(failure||doc.error)&&<p role="alert">{failure||doc.error}{doc.error&&<button onClick={()=>void doc.reload()}>Reload saved board</button>}</p>}
    <div className="board-editbar"><small>Drag connectors to link cards. Select a line to edit or remove it. Dashed lines are AI suggestions.</small>
      {state.hiddenNodes.length>0&&<button onClick={()=>edit(s=>({...s,hiddenNodes:[]}))}>Restore hidden cards ({state.hiddenNodes.length})</button>}
      {!!selected.length&&<><input aria-label="Group selected cards" placeholder="Group name" value={group} onChange={e=>setGroup(e.target.value)} maxLength={100}/><button onClick={()=>edit(s=>({...s,groups:{...s.groups,...Object.fromEntries(selected.map(id=>[id,group.trim()]))}}))}>Group {selected.length} cards</button></>}
    </div>
    {doc.ready&&<div className="board-flow"><ReactFlow nodes={nodes} edges={edges} nodeTypes={nodeTypes} onNodesChange={onNodesChange} onConnect={connect} onSelectionChange={selectionChanged}
      onNodeDragStop={(_,node,moved)=>edit(s=>({...s,positions:{...s.positions,...Object.fromEntries((moved.length?moved:[node]).map(n=>[n.id,n.position]))}}))}
      onEdgesDelete={deleted=>removeEdges(deleted.map(e=>e.id))} onNodesDelete={deleted=>edit(s=>({...s,hiddenNodes:[...new Set([...s.hiddenNodes,...deleted.map(n=>n.id)])]}))}
      onEdgeClick={(_,edge)=>{const link=links.find(e=>e.id===edge.id);setEdgeId(edge.id);setLabel(link?.label||"");setReason(link?.reason||"");}}
      onPaneClick={()=>setEdgeId(null)} fitView fitViewOptions={{padding:.2,maxZoom:1}} minZoom={.15} maxZoom={2.5} deleteKeyCode={["Backspace","Delete"]}>
      <Background gap={24} size={1}/><Controls showInteractive={false}/><MiniMap nodeColor="#79bca5" maskColor="rgba(10,20,15,.35)" pannable zoomable/>
    </ReactFlow></div>}
    {edgeId&&<div className="board-link-editor"><strong>Connection</strong><label>Label<input value={label} maxLength={240} onChange={e=>setLabel(e.target.value)}/></label><label>Why are these related?<textarea value={reason} maxLength={4000} onChange={e=>setReason(e.target.value)}/></label>
      <button disabled={!label.trim()} onClick={()=>{const edge=links.find(e=>e.id===edgeId);if(edge)edit(s=>({...s,links:[...s.links.filter(e=>e.id!==edgeId),{...edge,label:label.trim(),reason,origin:"user"}]}));setEdgeId(null);}}>Save connection</button>
      <button onClick={()=>removeEdges([edgeId])}>Remove connection</button><button onClick={()=>setEdgeId(null)}>Close</button><small>Board connections express your working interpretation. Original evidence and accepted project facts remain in history.</small>
    </div>}
  </div>;
}
