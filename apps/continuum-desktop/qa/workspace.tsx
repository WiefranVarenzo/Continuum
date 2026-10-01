// Browser-only fixture: no project files or model providers are contacted.
import React from "react";
import {createRoot} from "react-dom/client";
import {mockIPC,mockWindows} from "@tauri-apps/api/mocks";
import {Workspace} from "../src/Workspace";
import {sampleDocument} from "../src/sample";
import "../src/styles.css";
import "../src/desktop.css";
import "../src/diagrams.css";
const make=(id:string,title:string,type:string,details={})=>({entity:{id,title,entity_type:type,status:"available",version:1,updated_at:new Date().toISOString(),metadata:{}},details});
const session=make("session-demo","Comparing speech platforms","research_session",{objective:"Compare transcription quality, latency and pricing. Separate vendor claims from measured results."});
const items=[make("price","Pricing comparison","evidence",{annotation:"Published pricing; usage assumptions still need checking.",original_artifact_id:"image-demo",media_type:"image/png"}),make("trial","Latency experiment","evidence",{annotation:"A short trial, not yet a statistically robust result."}),make("question","Which platform fits our workflow?","research_question",{question:"Balance accuracy, latency, and cost."})];
const canvas=document.createElement("canvas");canvas.width=640;canvas.height=340;const ctx=canvas.getContext("2d")!;ctx.fillStyle="#eef5f1";ctx.fillRect(0,0,640,340);ctx.fillStyle="#203e32";ctx.font="bold 28px sans-serif";ctx.fillText("Speech platform research",35,55);ctx.font="21px sans-serif";["Pricing evidence · illustrative fixture","Platform A: validate usage assumptions","Platform B: check streaming support","Next: repeat the latency experiment"].forEach((s,i)=>ctx.fillText(s,35,115+i*52));const bytes=Array.from(atob(canvas.toDataURL().split(",")[1]),c=>c.charCodeAt(0));
const markdown="# Speech platform evaluation\n\n## What we know\n\nWe have collected pricing evidence and a preliminary latency experiment. **No winner has been verified yet.**\n\n## How the evidence supports a decision\n\n```mermaid\nflowchart LR\nsubgraph Sources\nP[Pricing evidence]\nT[Latency trial]\nend\nP --> C[Compare cost and response time]\nT --> C\nC --> V{Enough evidence?}\nV -->|No| R[Repeat measurements]\nV -->|Yes| D[Review recommendation]\n```\n\n## Comparison\n\n| Topic | Current evidence | Next step |\n|---|---|---|\n| Pricing | Published rates | Verify assumptions |\n| Latency | Preliminary trial | Repeat measurements |\n\n## Recommendation\n\nRun the same controlled experiment on both platforms before committing. This is an illustrative report, not real benchmark data.";
const documents=new Map<string,any>();documents.set("report:research:session-demo",{key:"report:research:session-demo",kind:"report",revision:1,payload:{markdown,sources:[{id:"price",title:"Pricing comparison"},{id:"trial",title:"Latency experiment"}],sourceSequence:128,status:"draft"}});
const pack={id:"pack-demo",source_ledger_sequence:128,estimated_tokens:1500,items:items.map((i,ordinal)=>({ordinal,source_id:i.entity.id,source_kind:i.entity.entity_type,classification:"internal",content:i})),omissions:[],unavailable_sources:[]};
mockWindows("main");mockIPC((command,payload:any)=>{
 if(command==="workspace_research_items")return {items:payload.sessionsOnly?[session]:items,next_offset:null};
 if(command==="load_workspace_document")return documents.get(payload.key)||null;
 if(command==="save_workspace_document"){const current=documents.get(payload.key);if((current?.revision||0)!==payload.expectedRevision)throw new Error("Revision conflict");const doc={...payload,revision:payload.expectedRevision+1,updated_at:new Date().toISOString()};documents.set(payload.key,doc);return doc;}
 if(command==="detect_ai_clients")return [{family:"codex",name:"Codex (fixture)",executable:"fixture"},{family:"hermes",name:"Hermes (fixture)",executable:"fixture"}];
 if(command==="workspace_thumbnail"||command==="workspace_media")return {artifact_id:"image-demo",media_type:"image/png",bytes};
 if(command==="current_project_state")return {as_of_ledger_sequence:128,now:{},since:[],next_actions:[],source_ids:[]};
 if(command==="recent_workspace_ai"||command==="list_mcp_report_drafts")return [];
 if(command.startsWith("list_"))return {items:[]};
 if(command==="run_workspace_assistant")return {job_id:crypto.randomUUID(),agent:payload.request.agent,image_count:1,pack,output:{markdown,notes:[{id:"price",title:"Published pricing",description:"A pricing source; validate its assumptions before estimating costs."}],connections:[{source:"price",target:"trial",label:"Compare cost with latency",reason:"Both inform the final tradeoff; this is a working suggestion."}]}};
 if(command==="inspect_workspace_memory")return pack;
 if(command==="export_workspace_markdown"){const report=documents.get(payload.key);const content=payload.format==="html"?payload.html:report.payload.markdown;const url=URL.createObjectURL(new Blob([content],{type:payload.format==="html"?"text/html":"text/plain"}));window.open(url,"_blank");return "synthetic-browser-preview";}
 return null;
},{shouldMockEvents:true});
localStorage.setItem("continuum.workspace.qa-project.session","session-demo");
const report={...sampleDocument,title:"Speech platform evaluation",blocks:sampleDocument.blocks.filter(b=>b.content.type!=="graph")};
createRoot(document.getElementById("root")!).render(<Workspace project={{project_id:"qa-project",name:"Continuum · UI test fixture",path:"/synthetic",status:"active",ledger_sequence:128,research:true,development:false,integrity_healthy:true}} capabilities={{research:true,development:false}} document={report} busy={false} settings={<p>Fixture settings</p>} onRefresh={async()=>{}} onExport={async()=>{}} onRepository={async()=>{throw new Error("Fixture only");}}/>);
