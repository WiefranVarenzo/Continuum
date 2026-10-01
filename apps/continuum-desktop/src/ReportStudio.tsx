import {useEffect, useState} from "react";
import {invoke} from "@tauri-apps/api/core";
import type {HumanDocument} from "./contracts";
import type {CheckpointScope} from "./contextContracts";
import {useProjectDocument,runAssistant,flushProjectDocuments,type AssistantAgent} from "./projectDocuments";
import {MarkdownReading} from "./MarkdownReading";
import {renderMarkdown,standaloneReport,type ReportSource} from "./markdownReport";
import {mermaidSource} from "./MermaidDiagram";
import {HumanDocumentView} from "./HumanDocumentView";
import {copyText} from "./workspaceBridge";

export interface ReportDraft {markdown:string;sources:ReportSource[];sourceSequence:number;status:"draft"|"reviewed";agent?:string;contextPackId?:string;origin?:"ai"|"mcp"|"manual"|"legacy";proposalId?:string;title?:string;coverage?:{sources:number;images:number;omissions:number}}
function legacyDraft(document:HumanDocument):ReportDraft {
  const lines=["# "+document.title];
  for(const block of document.blocks){const c=block.content;if(c.type==="citation_list"||c.type==="graph")continue;lines.push("\n## "+block.meta.title+"\n");
    if(c.type==="overview")lines.push(c.summary,...c.blockers.map(v=>"- Open: "+v),...c.next_actions.map(v=>"- Next: "+v));
    else if(c.type==="prose"||c.type==="callout")lines.push(c.text);
    else if(c.type==="research_synthesis")lines.push(c.summary,...c.key_points.map(v=>"- "+v),...c.limitations.map(v=>"- Limitation: "+v),...c.recommendations.map(v=>"- Recommendation: "+v));
    else if(c.type==="mermaid_diagram")lines.push("```mermaid\n"+mermaidSource(c.specification)+"\n```");
    else if(c.type==="table")lines.push(...c.rows.map(row=>"- "+Object.entries(row.cells).map(([k,v])=>k+": "+v).join(" · ")));
    else if(c.type==="timeline")lines.push(...c.entries.map(e=>"- "+e.label+": "+e.detail));
    else if(c.type==="key_value")lines.push(...c.facts.map(f=>"- "+f.key+": "+f.value));
    else if(c.type==="status_cards")lines.push(...c.cards.map(f=>"- "+f.label+": "+f.value));
  }
  return {markdown:lines.join("\n\n"),sources:document.citations.map(s=>({id:s.source_id,title:s.title,status:s.status})),sourceSequence:document.source_ledger_sequence,status:"draft",origin:"legacy"};
}
export function ReportStudio({projectId,scope,sessionId,document,agent}:{projectId:string;scope:CheckpointScope;sessionId:string|null;document:HumanDocument;agent:AssistantAgent}){
  const key="report:"+scope+":"+(sessionId||"project");
  const doc=useProjectDocument<ReportDraft>(projectId,key,"report",{markdown:"",sources:[],sourceSequence:0,status:"draft",origin:"manual"});
  const [editing,setEditing]=useState(false),[busy,setBusy]=useState(false),[error,setError]=useState(""),[notice,setNotice]=useState("");
  const [instructions,setInstructions]=useState("Write a comprehensive, evidence-backed report for another IT professional. Use clear explanations and useful Mermaid diagrams. Follow the language of this project.");
  const [proposed,setProposed]=useState<ReportDraft|null>(null);
  const [showOriginal,setShowOriginal]=useState(false);
  const [savedDrafts,setSavedDrafts]=useState<ReportDraft[]>([]),[loadingDrafts,setLoadingDrafts]=useState(false);
  const hasReport=!!doc.value.markdown.trim();
  async function refreshDrafts(){setLoadingDrafts(true);try {
    const [mcp,local]=await Promise.all([invoke<ReportDraft[]>("list_mcp_report_drafts",{projectId,scope,sessionId}),invoke<{payload:{mode:string;output:{markdown:string};pack:{items:{source_id:string;source_kind:string;content:Record<string,unknown>}[];id:string;source_ledger_sequence:number};agent:string;job_id:string}}[]>("recent_workspace_ai",{projectId,sessionId})]);
    setSavedDrafts([...mcp,...local.filter(d=>d.payload.mode==="report").map(({payload:p})=>({markdown:p.output.markdown,sources:p.pack.items.map(s=>({id:s.source_id,title:String((s.content.entity as Record<string,unknown>|undefined)?.title||s.content.title||s.source_kind)})),sourceSequence:p.pack.source_ledger_sequence,status:"draft" as const,origin:"ai" as const,agent:p.agent,contextPackId:p.pack.id,title:`Saved ${p.agent} draft`,proposalId:p.job_id}))]);
  }catch(cause){setError(String(cause));}finally{setLoadingDrafts(false);}}
  useEffect(()=>{void refreshDrafts();},[projectId,scope,sessionId]);
  async function copyMcpTask(){try {
    await copyText(`Use the connected Continuum MCP for project ${projectId}. Read its current context for scope ${scope}, research_session_id ${sessionId||"null"}. ${instructions}\nWrite the complete report as freeform Markdown with useful Mermaid diagrams and source citations. Do not use the application's legacy report sections as a required template. Submit it with the proposal submission tool using kind research_synthesis, scope ${scope}, a descriptive title, source_refs containing only the sources you read, and payload {"format":"markdown","research_session_id":${JSON.stringify(sessionId)},"markdown":"<the complete Markdown report>","source_sequence":<the context pack ledger sequence>}. Cite sources as [readable title](continuum://source/ID). Explain missing evidence; do not invent results. Leave it as a draft for the user to review in Reports.`);
    setNotice("Task copied. Paste it into your connected AI client, then refresh incoming drafts here after it submits the report.");
  }catch(cause){setError(String(cause));}}
  const unresolved=[...doc.value.markdown.matchAll(/continuum:\/\/source\/([a-zA-Z0-9_-]+)/g)].map(m=>m[1]).filter(id=>!doc.value.sources.some(s=>s.id===id));
  async function generate(){setBusy(true);setError("");try{
    const result=await runAssistant(projectId,scope,sessionId,agent,"report",instructions);
    setProposed({markdown:result.output.markdown,sources:result.pack.items.map(s=>({id:s.source_id,title:String((s.content.entity as Record<string,unknown>|undefined)?.title||s.content.title||s.source_kind),status:String((s.content.entity as Record<string,unknown>|undefined)?.status||"Source in AI context")})),sourceSequence:result.pack.source_ledger_sequence,status:"draft",origin:"ai",agent:result.agent,contextPackId:result.pack.id,coverage:{sources:result.pack.items.length,images:result.image_count,omissions:result.pack.omissions.length+result.pack.unavailable_sources.length}});
    setNotice("A new draft is ready below. Your current report has not been replaced.");
  }catch(cause){setError(String(cause));}finally{setBusy(false);}}
  async function exportReport(format:"html"|"markdown"){
    setBusy(true);setError("");try{doc.save(doc.ref.current);await flushProjectDocuments();const html=format==="html"?await standaloneReport(document.title,await renderMarkdown(doc.ref.current.markdown,doc.ref.current.sources)):null;
      const path=await invoke<string>("export_workspace_markdown",{projectId,key,format,html});setNotice("Saved: "+path);
    }catch(cause){setError(String(cause));}finally{setBusy(false);}
  }
  return <section className="report-studio"><div className="page-actions"><h1>Reports</h1><span className="status-chip">{!hasReport?"No report yet":doc.value.status==="reviewed"?"Reviewed by you":"Draft · check the evidence"}</span><button disabled={!doc.ready||busy} onClick={()=>setEditing(!editing)}>{editing?"Read report":hasReport?"Edit Markdown":"Write Markdown"}</button><button disabled={!doc.ready||busy||!hasReport} onClick={()=>void exportReport("markdown")}>Export .md</button><button disabled={!doc.ready||busy||!hasReport} onClick={()=>void exportReport("html")}>Export HTML</button></div>
    <p>One Markdown document, with a readable HTML view. {doc.saving?"Saving your edits…":"Edits and previous versions are stored in this project."}</p>
    <div className="page-actions"><button disabled={busy||!doc.ready} onClick={()=>void generate()}>{busy?"AI is preparing your report…":"Generate report with AI"}</button><small>Uses the selected agent and eligible project context. New drafts never overwrite this report without your confirmation.</small></div>
    {(error||doc.error)&&<p role="alert">{error||doc.error}</p>}{notice&&<p role="status">{notice}</p>}
    {!hasReport&&!editing&&<div className="report-empty"><h2>Turn your evidence into a clear report</h2><p>Ask the AI to explain this research or development project in its own structure, with source links and useful Mermaid diagrams. You can also write or paste Markdown yourself.</p><p>Choose the local assistant above, or send a task to your connected MCP client below. Your first report starts when you choose a draft or write it.</p></div>}
    {hasReport&&<p className="report-origin">{doc.value.origin==="ai"?`AI draft · ${doc.value.agent}`:doc.value.origin==="mcp"?"Draft received through MCP":doc.value.origin==="legacy"?"Imported legacy report · not AI-generated":doc.value.origin==="manual"?"Markdown authored by you":"Existing saved draft · original author was not recorded"}{doc.value.sourceSequence>0&&doc.value.sourceSequence<document.source_ledger_sequence?" · Newer project activity is available":""}</p>}
    <details className="assistant-request"><summary>Use a connected AI through MCP · incoming drafts ({savedDrafts.length})</summary><p>Your connected Codex, Claude or other MCP client can write and submit the full Markdown report. Allow proposal submission in AI connections first.</p><div className="page-actions"><button onClick={()=>void copyMcpTask()}>Copy task for connected AI</button><button disabled={loadingDrafts} onClick={()=>void refreshDrafts()}>{loadingDrafts?"Checking drafts…":"Refresh incoming drafts"}</button></div>{savedDrafts.map((draft,index)=><div className="page-actions" key={draft.proposalId||index}><span>{draft.title||"Incoming report"} · {draft.origin==="mcp"?"MCP":"Saved AI result"}</span><button disabled={busy} onClick={()=>setProposed(draft)}>Preview draft</button></div>)}</details>
    <details className="assistant-request"><summary>Write or improve a report with {agent==="hermes"?"Hermes":"Codex"}</summary><label>What should the report explain?<textarea value={instructions} onChange={e=>setInstructions(e.target.value)} maxLength={12000}/></label><button disabled={busy||!doc.ready||!instructions.trim()} onClick={()=>void generate()}>{busy?"Preparing the report…":"Generate a new draft"}</button><small>The AI receives a bounded project context and available image previews. Review the result before using it as a final report.</small></details>
    {proposed&&<div className="report-proposal"><h2>{proposed.origin==="legacy"?"Legacy report import":"New AI draft"}</h2>{proposed.coverage&&<p>Based on {proposed.coverage.sources} context items and {proposed.coverage.images} image previews. {proposed.coverage.omissions} sources were omitted or unavailable; check limitations before reviewing.</p>}<button disabled={!doc.ready||!!doc.error} onClick={()=>{doc.save(proposed);setProposed(null);setEditing(false);}}>Use this draft</button><button onClick={()=>setProposed(null)}>Keep current report</button><MarkdownReading markdown={proposed.markdown} sources={proposed.sources}/></div>}
    {!!unresolved.length&&<p role="alert">Some source links cannot be matched to this report's saved evidence. Correct them before marking this report reviewed.</p>}
    {doc.ready&&(editing?<label className="markdown-editor">Markdown source<textarea spellCheck={false} value={doc.value.markdown} maxLength={300000} onChange={e=>doc.save({...doc.ref.current,markdown:e.target.value,status:"draft"},true)}/></label>:hasReport?<MarkdownReading markdown={doc.value.markdown} sources={doc.value.sources}/>:null)}
    <div className="page-actions"><button disabled={!doc.ready||!!unresolved.length||!doc.value.markdown.trim()||!!doc.error} onClick={()=>doc.save({...doc.ref.current,status:"reviewed"})}>Mark as reviewed by me</button>{doc.revision>1&&<button onClick={()=>void invoke<ReportDraft>("load_workspace_revision",{projectId,key,revision:doc.revision-1}).then(previous=>doc.save(previous)).catch(cause=>setError(String(cause)))}>Restore previous saved version</button>}</div>
    <details onToggle={e=>setShowOriginal(e.currentTarget.open)}><summary>Legacy structured report · optional reference</summary>{showOriginal&&<><p>This reference is generated by the application. Import it only if you want it as a starting draft.</p><button onClick={()=>setProposed(legacyDraft(document))}>Preview legacy import</button><HumanDocumentView document={document}/></>}</details>
  </section>;
}
