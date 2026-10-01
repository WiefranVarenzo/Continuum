import {useState} from "react";
import type {CheckpointScope} from "./contextContracts";
import {useProjectDocument,runAssistant,type AssistantAgent} from "./projectDocuments";
import {MarkdownReading} from "./MarkdownReading";
import type {ReportSource} from "./markdownReport";

interface Message {id:string;role:"user"|"assistant";text:string;sources?:ReportSource[];time:string;agent?:string;contextPackId?:string}
export function Messages({projectId,scope,sessionId,agent}:{projectId:string;scope:CheckpointScope;sessionId:string|null;agent:AssistantAgent}){
  const doc=useProjectDocument<{messages:Message[]}>(projectId,"messages:"+scope+":"+(sessionId||"project"),"messages",{messages:[]});
  const [prompt,setPrompt]=useState(""),[busy,setBusy]=useState(false),[error,setError]=useState("");
  async function send(text=prompt){if(!text.trim()||busy||!doc.ready)return;setBusy(true);setError("");setPrompt("");
    const history=doc.ref.current.messages.slice(-12).map(m=>({role:m.role,text:m.text.slice(0,1800)}));
    try{const result=await runAssistant(projectId,scope,sessionId,agent,"chat",text,history);doc.save({messages:[...doc.ref.current.messages,{id:crypto.randomUUID(),role:"user",text,time:new Date().toISOString()},{id:result.job_id,role:"assistant",text:result.output.markdown,time:new Date().toISOString(),agent:result.agent,contextPackId:result.pack.id,sources:result.pack.items.map(s=>({id:s.source_id,title:String((s.content.entity as Record<string,unknown>|undefined)?.title||s.content.title||s.source_kind)}))}]});}
    catch(cause){setError(String(cause));setPrompt(text);}finally{setBusy(false);}
  }
  return <section className="messages-page"><div className="page-actions"><h1>Messages</h1><span>With {agent==="hermes"?"Hermes":"Codex"}</span></div><p>Ask about this {sessionId?"research session":"project"}. Answers use saved context and cite their sources; chatting does not change project facts.</p>
    {!doc.value.messages.length&&<div className="message-starters"><button disabled={!doc.ready||busy} onClick={()=>void send("Kemarin kita sampai mana? Jelaskan yang sudah selesai, yang belum pasti, dan langkah berikutnya berdasarkan bukti proyek.")}>Where did we stop?</button><button disabled={!doc.ready||busy} onClick={()=>void send("Apa yang masih belum diuji atau dibuktikan dalam proyek ini?")}>What still needs evidence?</button></div>}
    <div className="message-list" aria-live="polite">{doc.value.messages.map(m=><article key={m.id} className={"message "+m.role}><div><strong>{m.role==="user"?"You":m.agent==="hermes"?"Hermes":"Codex"}</strong><time>{new Date(m.time).toLocaleString()}</time></div>{m.role==="user"?<p>{m.text}</p>:<MarkdownReading markdown={m.text} sources={m.sources}/>}</article>)}{busy&&<p role="status">Reading the saved project context and preparing an answer…</p>}</div>
    {(error||doc.error)&&<p role="alert">{error||doc.error}</p>}
    <form className="message-composer" onSubmit={e=>{e.preventDefault();void send();}}><label>Message<textarea value={prompt} onChange={e=>setPrompt(e.target.value)} maxLength={12000} placeholder="Ask about evidence, decisions, or the next step…"/></label><button disabled={busy||!doc.ready||!prompt.trim()||!!doc.error}>Send</button></form>
  </section>;
}
