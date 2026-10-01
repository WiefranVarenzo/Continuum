import {useEffect,useState} from "react";
import {invoke} from "@tauri-apps/api/core";
import type {WorkspaceItem} from "./workspaceBridge";
import type {CapturePreview} from "./captureContracts";

export function EvidenceMedia({item,projectId,thumbnail=false}:{item:WorkspaceItem;projectId:string;thumbnail?:boolean}) {
  const [src,setSrc]=useState(""),[type,setType]=useState(""),[error,setError]=useState(""),[opened,setOpened]=useState(false);
  const artifact=item.details.original_artifact_id;
  const hinted=String(item.details.media_type||item.details.mime_type||item.entity.metadata?.media_type||"");
  const isMedia=/^(video|audio)\//.test(hinted)||["video","audio","recording","recording_segment"].includes(String(item.details.kind));
  useEffect(()=>{
    setSrc("");setError("");if(typeof artifact!=="string")return;
    if(thumbnail&&isMedia&&!opened)return;
    let active=true,url="";
    const command=thumbnail&&!isMedia?"workspace_thumbnail":"workspace_media";
    void invoke<CapturePreview>(command,{projectId,artifactId:artifact}).then(result=>{
      if(!active)return;url=URL.createObjectURL(new Blob([new Uint8Array(result.bytes)],{type:result.media_type}));setSrc(url);setType(result.media_type);
    }).catch(cause=>{if(active)setError(String(cause));});
    return()=>{active=false;if(url)URL.revokeObjectURL(url);};
  },[artifact,projectId,thumbnail,isMedia,opened]);
  if(typeof artifact!=="string")return null;
  if(thumbnail&&isMedia&&!opened)return <button className="media-open nodrag nopan" onClick={()=>setOpened(true)}>▶ Open {hinted.startsWith("audio/")?"audio":"recording"}</button>;
  if(error)return <div className="media-error">Preview unavailable. The original is saved.<details><summary>Details</summary>{error}</details></div>;
  if(!src)return <div className="image-placeholder">Loading preview…</div>;
  if(type.startsWith("video/"))return <video className="nodrag nopan" src={src} controls preload="metadata" onError={()=>setError("This recording needs a compatible codec or complete recording fragments.")} />;
  if(type.startsWith("audio/"))return <audio className="nodrag nopan" src={src} controls preload="metadata" onError={()=>setError("This recording cannot be decoded by the current media runtime.")} />;
  return <img src={src} alt={item.entity.title} loading="lazy"/>;
}
