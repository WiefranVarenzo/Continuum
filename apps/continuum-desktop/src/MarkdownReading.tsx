import {useEffect,useRef,useState} from "react";
import {renderMarkdown,wireDiagrams,markdownStyles,type ReportSource} from "./markdownReport";
export function MarkdownReading({markdown,sources}:{markdown:string;sources?:ReportSource[]}){
  const [html,setHtml]=useState(""),[error,setError]=useState("");const root=useRef<HTMLDivElement>(null);
  const sourcePrefix=useRef("reading-"+crypto.randomUUID());
  useEffect(()=>{let active=true;void renderMarkdown(markdown,sources).then(text=>{if(active){setHtml(text.replaceAll('id="source-',`id="${sourcePrefix.current}-source-`).replaceAll('href="#source-',`href="#${sourcePrefix.current}-source-`));setError("");}}).catch(cause=>{if(active)setError(String(cause));});return()=>{active=false;};},[markdown,sources]);
  useEffect(()=>{if(root.current)return wireDiagrams(root.current);},[html]);
  return <><style>{markdownStyles}</style>{error&&<p role="alert">{error}</p>}<div className="markdown-reading" ref={root} dangerouslySetInnerHTML={{__html:html}}/></>;
}
