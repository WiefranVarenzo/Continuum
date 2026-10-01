import {Marked} from "marked";
import DOMPurify from "dompurify";

export const escapeHtml=(s:string)=>s.replace(/[&<>"']/g,c=>({"&":"&amp;","<":"&lt;",">":"&gt;",'"':"&quot;","'":"&#39;"}[c]!));
export interface ReportSource {id:string;title:string;status?:string}
let rendering=Promise.resolve();
export async function renderMarkdown(markdown:string,sources:ReportSource[]=[]):Promise<string>{
  const diagrams:string[]=[];
  const parser=new Marked({gfm:true,breaks:false,renderer:{
    html({text}){return escapeHtml(text);},
    code({text,lang}){if(lang?.trim()==="mermaid"){const index=diagrams.push(text)-1;return '<div class="md-diagram-pending" data-diagram="'+index+'"></div>';}return '<pre><code>'+escapeHtml(text)+'</code></pre>';},
    link({href,tokens}){
      const title=this.parser.parseInline(tokens);const match=href.match(/^continuum:\/\/source\/([a-zA-Z0-9_-]+)$/);
      if(match)return '<a href="#source-'+match[1]+'">'+title+'</a>';
      if(!/^(https?:\/\/|mailto:|#)/i.test(href))return title;
      return '<a rel="noopener noreferrer" href="'+escapeHtml(href)+'">'+title+'</a>';
    },
    image({href,text}){return /^data:image\/(png|jpeg|webp);base64,[a-zA-Z0-9+/=]+$/.test(href)?'<img src="'+href+'" alt="'+escapeHtml(text)+'" loading="lazy"/>':'<span class="md-media-reference">Image reference: '+escapeHtml(text)+'</span>';},
  }});
  const html=DOMPurify.sanitize(await parser.parse(markdown),{FORBID_TAGS:["style","iframe","form","input"],FORBID_ATTR:["style"]});
  const container=document.createElement("div");container.innerHTML=html;
  for(const table of container.querySelectorAll("table")){const wrap=document.createElement("div");wrap.className="md-table-scroll";table.replaceWith(wrap);wrap.append(table);}
  for(const block of container.querySelectorAll<HTMLElement>("[data-diagram]")){
    const source=diagrams[Number(block.dataset.diagram)];
    if(!source||source.length>24000||/%%\s*\{|^\s*(click|linkStyle.*url)\b/im.test(source)){block.textContent="Diagram configuration is unsupported. Edit the Mermaid block.";continue;}
    const work=rendering.then(async()=>{
      try{const {default:mermaid}=await import("mermaid");
        mermaid.initialize({startOnLoad:false,securityLevel:"strict",theme:"base",themeVariables:{background:"#14211b",primaryColor:"#213c32",primaryTextColor:"#edf6f0",primaryBorderColor:"#77c6aa",lineColor:"#8eada0",secondaryColor:"#24364b",tertiaryColor:"#302d44",clusterBkg:"#1a2c24",clusterBorder:"#527967",edgeLabelBackground:"#14211b",textColor:"#edf6f0",titleColor:"#edf6f0",fontFamily:"system-ui,sans-serif"},htmlLabels:false,flowchart:{htmlLabels:false,useMaxWidth:false,curve:"basis",nodeSpacing:50,rankSpacing:70},maxTextSize:24000,maxEdges:250});
        const result=await mermaid.render("md-"+crypto.randomUUID().replaceAll("-",""),source);
        const svg=DOMPurify.sanitize(result.svg,{USE_PROFILES:{svg:true,svgFilters:true},FORBID_TAGS:["foreignObject","script","a"]});
        block.className="md-diagram";block.innerHTML='<div class="md-diagram-tools"><span>Diagram · drag to explore</span><button type="button" data-zoom="out" aria-label="Zoom diagram out">−</button><button type="button" data-zoom="in" aria-label="Zoom diagram in">+</button><button type="button" data-zoom="fit">Fit</button><button type="button" data-zoom="full">Fullscreen</button></div><div class="md-diagram-view" tabindex="0"><div class="md-diagram-svg">'+svg+'</div></div><details><summary>Mermaid source</summary><pre><code>'+escapeHtml(source)+'</code></pre></details>';
      }catch{block.className="md-diagram-error";block.innerHTML='<p>Diagram could not be rendered. The report remains readable; edit this Mermaid block.</p><pre><code>'+escapeHtml(source)+'</code></pre>';}
    });rendering=work.catch(()=>undefined);await work;
  }
  if(sources.length)container.insertAdjacentHTML("beforeend",'<section class="md-sources"><h2>Sources</h2><ol>'+sources.map(s=>'<li id="source-'+escapeHtml(s.id)+'"><strong>'+escapeHtml(s.title)+'</strong>'+ (s.status?' <small>'+escapeHtml(s.status)+'</small>':'')+'</li>').join("")+'</ol></section>');
  return container.innerHTML;
}

// Shared by the in-app preview and standalone HTML. No external requests.
export function wireDiagrams(root:HTMLElement|Document){
  const cleanups:(()=>void)[]=[];
  root.querySelectorAll<HTMLElement>(".md-diagram").forEach(shell=>{
    const view=shell.querySelector<HTMLElement>(".md-diagram-view")!,svg=shell.querySelector<SVGSVGElement>("svg");if(!svg)return;
    const box=svg.viewBox?.baseVal;const width=box?.width||800,height=box?.height||400;
    let zoom=1,manualZoom=false,drag:{x:number;y:number;left:number;top:number}|null=null;
    const size=()=>{svg.style.maxWidth="none";svg.style.width=width*zoom+"px";svg.style.height=height*zoom+"px";};
    const fit=()=>{zoom=Math.max(.1,Math.min(1,(view.clientWidth-24)/width,(view.clientHeight-24)/height));size();view.scrollTo?.(0,0);};fit();
    const resize=typeof ResizeObserver==="undefined"?null:new ResizeObserver(()=>{if(!manualZoom&&view.clientWidth>0)fit();});resize?.observe(view);
    const full=()=>{manualZoom=false;fit();};document.addEventListener("fullscreenchange",full);
    const click=(event:Event)=>{const target=(event.target as HTMLElement).closest<HTMLElement>("[data-zoom]");if(!target)return;switch(target.dataset.zoom){case"in":manualZoom=true;zoom=Math.min(4,zoom*1.25);size();break;case"out":manualZoom=true;zoom=Math.max(.1,zoom/1.25);size();break;case"fit":manualZoom=false;fit();break;case"full":void (document.fullscreenElement?document.exitFullscreen():shell.requestFullscreen())?.catch(()=>undefined);break;}};
    const down=(event:PointerEvent)=>{if(event.button!==0)return;drag={x:event.clientX,y:event.clientY,left:view.scrollLeft,top:view.scrollTop};view.setPointerCapture?.(event.pointerId);};
    const move=(event:PointerEvent)=>{if(drag){view.scrollLeft=drag.left-event.clientX+drag.x;view.scrollTop=drag.top-event.clientY+drag.y;}};
    const up=()=>{drag=null;};
    shell.addEventListener("click",click);view.addEventListener("pointerdown",down);view.addEventListener("pointermove",move);view.addEventListener("pointerup",up);view.addEventListener("pointercancel",up);
    cleanups.push(()=>{resize?.disconnect();document.removeEventListener("fullscreenchange",full);shell.removeEventListener("click",click);view.removeEventListener("pointerdown",down);view.removeEventListener("pointermove",move);view.removeEventListener("pointerup",up);view.removeEventListener("pointercancel",up);});
  });return()=>cleanups.forEach(f=>f());
}

export const markdownStyles=`.markdown-reading{font:16px/1.75 system-ui,sans-serif;color:var(--text,#eaf1ed);overflow-wrap:anywhere;max-width:1000px;margin:auto}.markdown-reading h1{font-size:30px;line-height:1.25;margin:1rem 0 2rem}.markdown-reading h2{font-size:23px;margin:2.5rem 0 1rem}.markdown-reading h3{font-size:18px;margin-top:1.6rem}.markdown-reading p,.markdown-reading ul,.markdown-reading ol{max-width:85ch}.markdown-reading a{color:var(--brand,#77d4b2)}.markdown-reading pre{overflow:auto;padding:18px;border-radius:8px;background:var(--bg,#101713);white-space:pre;max-height:500px}.markdown-reading code{font:13px/1.6 monospace}.markdown-reading blockquote{border-left:3px solid var(--brand,#77d4b2);padding-left:18px;color:var(--muted,#a7b8ac)}.markdown-reading img{max-width:100%;max-height:650px;object-fit:contain}.md-table-scroll{overflow:auto;margin:20px 0}.md-table-scroll table{border-collapse:collapse;min-width:650px;width:100%}.md-table-scroll td,.md-table-scroll th{padding:12px 16px;border:1px solid var(--line,#34453b);min-width:120px;vertical-align:top}.md-diagram{margin:24px 0;border:1px solid var(--line,#34453b);border-radius:12px;overflow:hidden;background:var(--panel,#18241d)}.md-diagram-tools{display:flex;align-items:center;gap:8px;flex-wrap:wrap;padding:12px}.md-diagram-tools span{margin-right:auto;font-size:12px}.md-diagram-tools button{font:inherit;padding:6px 12px;background:var(--bg,#101713);color:var(--text,#eaf1ed);border:1px solid var(--line,#34453b);border-radius:7px;cursor:pointer}.md-diagram-view{height:460px;overflow:auto;background:#14211b;cursor:grab;touch-action:none}.md-diagram-svg{width:max-content;min-width:100%;min-height:100%;display:grid;place-items:center;padding:12px}.md-diagram details{padding:12px}.md-diagram:fullscreen{background:var(--panel,#18241d)}.md-diagram:fullscreen .md-diagram-view{height:85vh}.md-sources{border-top:1px solid var(--line,#34453b);margin-top:40px}.md-sources small{color:var(--muted,#a7b8ac)}.md-media-reference,.md-diagram-error{color:var(--muted,#a7b8ac)}@media print{.md-diagram-tools{display:none}.md-diagram-view{height:auto;overflow:visible}.markdown-reading{color:#111}.md-diagram-svg svg{max-width:100%!important;height:auto!important}}`;
export async function standaloneReport(title:string,html:string){
  const script='('+wireDiagrams.toString()+')(document);';
  const hash=btoa(String.fromCharCode(...new Uint8Array(await crypto.subtle.digest("SHA-256",new TextEncoder().encode(script)))));
  return '<!DOCTYPE html><html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><meta http-equiv="Content-Security-Policy" content="default-src \'none\'; img-src data:; style-src \'unsafe-inline\'; script-src \'sha256-'+hash+'\'; base-uri \'none\'; form-action \'none\'"><title>'+escapeHtml(title)+'</title><style>:root{--bg:#101713;--panel:#18241d;--text:#eaf1ed;--line:#34453b;--brand:#77d4b2;--muted:#a7b8ac}body{margin:0;background:var(--bg);padding:40px 24px}'+markdownStyles+'</style></head><body><main class="markdown-reading">'+html+'</main><script>'+script+'</script></body></html>';
}
