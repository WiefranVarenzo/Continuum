import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { ContextPack, CheckpointScope } from "./contextContracts";

export interface ProjectDocument<T> { key:string; kind:string; revision:number; payload:T; updated_at:string }
const pending = new Set<Promise<unknown>>();
const failures = new Map<string,string>();
const scheduled=new Map<string,()=>void>();
let activeRequests=0;
export async function flushProjectDocuments() {
  for(const flush of [...scheduled.values()])flush();
  while (pending.size) await Promise.allSettled([...pending]);
  if (failures.size) throw new Error([...failures.values()].join("\n"));
}
export function useProjectDocument<T>(projectId:string,key:string,kind:string,initial:T) {
  const [value,setValue]=useState<T>(initial);
  const [ready,setReady]=useState(false);
  const [error,setError]=useState("");
  const [saving,setSaving]=useState(false);
  const [revision,setRevision]=useState(0);
  const ref=useRef(initial), version=useRef(0), queue=useRef(Promise.resolve()), mounted=useRef(true), blocked=useRef(false);
  const timer=useRef<ReturnType<typeof setTimeout>|null>(null);
  const identity=`${projectId}:${key}`;
  async function reload() {
    try {
      scheduled.get(identity)?.();
      await queue.current;
      const doc=await invoke<ProjectDocument<T>|null>("load_workspace_document",{projectId,key});
      ref.current=doc?.payload??initial;version.current=doc?.revision??0;blocked.current=false;failures.delete(identity);
      if(mounted.current){setValue(ref.current);setRevision(version.current);setError("");setReady(true);}
    } catch(cause) {if(mounted.current)setError(String(cause));}
  }
  useEffect(()=>{mounted.current=true;void reload();return()=>{mounted.current=false;scheduled.get(identity)?.();};},[projectId,key]); // key changes are mounted with React keys by callers
  function save(next:T,debounce=false) {
    if(!ready||blocked.current)return;
    ref.current=next;setValue(next);setSaving(true);
    if(timer.current)clearTimeout(timer.current);
    scheduled.delete(identity);
    const persist=()=>{
    if(timer.current)clearTimeout(timer.current);
    scheduled.delete(identity);timer.current=null;
    const task=queue.current.then(async()=>{
      if(blocked.current)return;
      try {
        const doc=await invoke<ProjectDocument<T>>("save_workspace_document",{projectId,key,kind,expectedRevision:version.current,payload:next});
        version.current=doc.revision;failures.delete(identity);
        if(mounted.current){setRevision(doc.revision);setError("");}
      } catch(cause) {
        const message=String(cause);blocked.current=true;failures.set(identity,message);if(mounted.current)setError(message);
      }
    });
    queue.current=task;pending.add(task);
    void task.finally(()=>{pending.delete(task);if(mounted.current&&queue.current===task)setSaving(false);});
    };
    if(debounce){scheduled.set(identity,persist);timer.current=setTimeout(persist,800);}else persist();
  }
  return {value,ref,save,ready,error,saving,revision,reload};
}
const closeTasks=new Set<()=>Promise<void>>();
export function registerCloseTask(task:()=>Promise<void>){closeTasks.add(task);return()=>{closeTasks.delete(task);};}
export async function prepareWorkspaceClose(){if(activeRequests)throw new Error("An AI request is still running. Wait for its result before closing.");for(const task of closeTasks)await task();await flushProjectDocuments();}
export type AssistantAgent="codex"|"hermes";
export interface AssistantOutput { markdown:string; connections:{source:string;target:string;label:string;reason:string}[]; notes:{id:string;title:string;description:string}[] }
export interface AssistantResult {output:AssistantOutput;pack:ContextPack;job_id:string;agent:AssistantAgent;image_count:number}
export function runAssistant(projectId:string,scope:CheckpointScope,sessionId:string|null,agent:AssistantAgent,mode:"chat"|"board"|"report",prompt:string,history:unknown[]=[]) {
  activeRequests++;
  return flushProjectDocuments().then(()=>invoke<AssistantResult>("run_workspace_assistant",{request:{project_id:projectId,scope,session_id:sessionId,agent,mode,prompt,history}})).finally(()=>{activeRequests--;});
}
