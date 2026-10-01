import {useState} from "react";
import {invoke} from "@tauri-apps/api/core";
import type {CheckpointScope,ContextPack} from "./contextContracts";
import {useProjectDocument} from "./projectDocuments";
import {MarkdownReading} from "./MarkdownReading";
import type {ProjectDocument,AssistantResult} from "./projectDocuments";
export function MemoryPanel({projectId,scope,sessionId,onBookmark}:{projectId:string;scope:CheckpointScope;sessionId:string|null;onBookmark:()=>Promise<void>}){
  const [pack,setPack]=useState<ContextPack|null>(null),[error,setError]=useState(""),[busy,setBusy]=useState(false);
  const [runs,setRuns]=useState<ProjectDocument<AssistantResult&{mode:string;working_notes:unknown}>[]>([]);
  const bookmark=useProjectDocument<{note:string;next:string;time?:string}>(projectId,"bookmark:"+(sessionId||scope),"preferences",{note:"",next:""});
  async function inspect(){setBusy(true);try{setPack(await invoke<ContextPack>("inspect_workspace_memory",{projectId,scope,sessionId,task:"Resume this project: progress, evidence, decisions and next actions"}));setRuns(await invoke("recent_workspace_ai",{projectId,sessionId}));setError("");}catch(cause){setError(String(cause));}finally{setBusy(false);}}
  return <section className="memory-panel"><h2>Resume & memory</h2><p>Your project keeps original sources, versioned board edits, reports, and conversations. An AI receives a selected context pack, not an unlimited or infallible memory.</p>
    <label>Where did you stop? <small>Optional — your work is saved even if left blank.</small><textarea disabled={!bookmark.ready} value={bookmark.value.note} maxLength={8000} onChange={e=>bookmark.save({...bookmark.ref.current,note:e.target.value},true)}/></label>
    <label>Next step<textarea disabled={!bookmark.ready} value={bookmark.value.next} maxLength={4000} onChange={e=>bookmark.save({...bookmark.ref.current,next:e.target.value},true)}/></label>
    <div className="page-actions"><button disabled={!bookmark.ready||busy} onClick={()=>{bookmark.save({...bookmark.ref.current,time:new Date().toISOString()});void onBookmark().catch(c=>setError(String(c)));}}>Save bookmark</button><button disabled={busy} onClick={()=>void inspect()}>{busy?"Preparing context…":"Inspect AI context"}</button></div>
    {(error||bookmark.error)&&<p role="alert">{error||bookmark.error}</p>}
    {runs.length>0&&<><h3>Recent AI results · saved even if you leave the page</h3>{runs.map(run=><details key={run.key}><summary>{run.payload.mode} · {run.payload.agent} · {new Date(run.updated_at).toLocaleString()}</summary><MarkdownReading markdown={run.payload.output.markdown}/><details><summary>Exact working notes sent to this request</summary><pre>{JSON.stringify(run.payload.working_notes,null,2)}</pre></details><details><summary>Exact context pack for this request</summary><pre>{JSON.stringify(run.payload.pack,null,2)}</pre></details></details>)}</>}
    {pack&&<><p>{pack.items.length} included sources · approximately {pack.estimated_tokens.toLocaleString()} tokens · {pack.omissions.length} omissions. Private or oversized content can be excluded. Actual requests also include your prompt and bounded image previews.</p>{pack.items.map(s=><details key={s.ordinal}><summary>{String((s.content.entity as Record<string,unknown>|undefined)?.title||s.content.title||s.source_kind)} · {s.classification}</summary><pre>{JSON.stringify(s.content,null,2)}</pre></details>)}<details><summary>Excluded sources and limitations</summary><pre>{JSON.stringify({omissions:pack.omissions,unavailable:pack.unavailable_sources},null,2)}</pre></details></>}
  </section>;
}
