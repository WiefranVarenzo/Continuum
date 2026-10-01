import {describe,it,expect,vi} from "vitest";
import {renderMarkdown,standaloneReport,wireDiagrams} from "./markdownReport";
vi.mock("mermaid",()=>({default:{initialize:vi.fn(),render:vi.fn().mockResolvedValue({svg:'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 80"><text>A → B</text></svg>'})}}));
describe("Markdown-first reports",()=>{
  it("refits when its viewport appears or resizes, while preserving manual zoom",async()=>{
    let resized!:()=>void;const disconnect=vi.fn();
    vi.stubGlobal("ResizeObserver",class {constructor(callback:()=>void){resized=callback;}observe(){}disconnect=disconnect;});
    const host=document.createElement("div");host.innerHTML=await renderMarkdown("```mermaid\nflowchart LR\nA-->B\n```");
    const view=host.querySelector<HTMLElement>(".md-diagram-view")!;let width=0;
    Object.defineProperty(view,"clientWidth",{get:()=>width});Object.defineProperty(view,"clientHeight",{value:460});
    const cleanup=wireDiagrams(host);const svg=host.querySelector<SVGSVGElement>("svg")!;const hidden=svg.style.width;
    width=700;resized();expect(svg.style.width).not.toBe(hidden);
    host.querySelector<HTMLButtonElement>('[data-zoom="in"]')!.click();const manual=svg.style.width;width=500;resized();expect(svg.style.width).toBe(manual);
    cleanup();expect(disconnect).toHaveBeenCalledOnce();vi.unstubAllGlobals();
  });
  it("renders headings, readable tables, source links and Mermaid controls",async()=>{
    const html=await renderMarkdown('# Findings\n\n[Price](continuum://source/e1)\n\n|Model|Result|\n|---|---|\n|A|Pass|\n\n```mermaid\nflowchart LR\nA-->B\n```',[{id:"e1",title:"Pricing evidence"}]);
    expect(html).toContain('<h1>Findings</h1>');expect(html).toContain('md-table-scroll');expect(html).toContain('id="source-e1"');expect(html).toContain('aria-label="Zoom diagram in"');expect(html).toContain('<svg');
  });
  it("escapes active HTML, disallows executable links and remote image tracking",async()=>{
    const html=await renderMarkdown('<script>alert(1)</script>\n\n[bad](javascript:alert%281%29)\n\n![tracker](https://tracking.test/pixel.png)');
    expect(html).not.toContain('<script');expect(html).not.toContain('href="javascript');expect(html).not.toContain('<img');
  });
  it("does not execute Mermaid configuration directives",async()=>{expect(await renderMarkdown('```mermaid\n%%{init:{}}%%\nflowchart LR\nA-->B\n```')).toContain('unsupported');});
  it("exports a self-contained HTML document with working controls and a hashed script",async()=>{
    vi.stubGlobal("crypto",{randomUUID:crypto.randomUUID.bind(crypto),subtle:{digest:async()=>new ArrayBuffer(32)}});
    const html=await standaloneReport("Test <report>",await renderMarkdown("# Report\n\n```mermaid\nflowchart LR\nA-->B\n```"));
    expect(html).toContain("<!DOCTYPE html>");expect(html).toContain("Test &lt;report&gt;");expect(html).toContain("script-src 'sha256-");expect(html).not.toContain('src="https://');
    const parsed=new DOMParser().parseFromString(html,"text/html");
    new Function("document",parsed.querySelector("script")!.textContent!)(parsed);
    const svg=parsed.querySelector("svg")!;const before=svg.style.width;
    parsed.querySelector<HTMLButtonElement>('[data-zoom="in"]')!.click();expect(svg.style.width).not.toBe(before);
    vi.unstubAllGlobals();
  });
});
