import { useEffect, useId, useMemo, useState, useRef } from "react";
import type { MermaidSpecification } from "./contracts";

function label(value: string): string {
  return value.replace(/[\r\n]+/g, " ").replace(/["<>`{}|\[\]\\#]/g, " ").slice(0, 180);
}

function nodeTone(kind: string, status: string): string {
  const value = `${kind} ${status}`.toLowerCase();
  if (/(failed|blocked|rejected|danger)/.test(value)) return "attention";
  if (/(checkpoint|context|continuity)/.test(value)) return "continuity";
  if (/(decision|requirement)/.test(value)) return "decision";
  if (/(change|code|repository|commit|test|development)/.test(value)) return "development";
  if (/(feedback|learning)/.test(value)) return "feedback";
  if (/(question|evidence|experiment|result|finding|research)/.test(value)) return "research";
  return "neutral";
}

export function mermaidSource(specification: MermaidSpecification): string {
  const aliases = new Map(specification.nodes.map((node, index) => [node.id, `n${index}`]));
  const direction = specification.direction === "top_down" ? "TD" : "LR";
  const lines = [
    `flowchart ${direction}`,
    "  classDef research fill:#DDF7EA,stroke:#187A55,color:#123D2D,stroke-width:2px",
    "  classDef development fill:#E7EEFF,stroke:#3F5FC4,color:#1E326F,stroke-width:2px",
    "  classDef decision fill:#F1E8FF,stroke:#7651B5,color:#3E276B,stroke-width:2px",
    "  classDef continuity fill:#DDF4FA,stroke:#157A96,color:#174453,stroke-width:3px",
    "  classDef feedback fill:#FFF2CD,stroke:#A26A00,color:#563B00,stroke-width:2px",
    "  classDef attention fill:#FFE4E6,stroke:#B83243,color:#6C1722,stroke-width:2px",
    "  classDef neutral fill:#EFF3F0,stroke:#66736A,color:#27322B,stroke-width:2px",
  ];
  for (const node of specification.nodes) {
    lines.push(`  ${aliases.get(node.id)}["${label(node.label)}"]:::${nodeTone(node.kind, node.status)}`);
  }
  for (const edge of specification.edges) {
    const from = aliases.get(edge.source);
    const to = aliases.get(edge.target);
    if (from && to) lines.push(`  ${from} -->|"${label(edge.label)}"| ${to}`);
  }
  return lines.join("\n");
}

export function MermaidDiagram({ specification }: { specification: MermaidSpecification }) {
  const reactId = useId();
  const diagramId = `continuum-${reactId.replace(/[^a-zA-Z0-9_-]/g, "")}`;
  const [direction, setDirection] = useState(specification.direction);
  const source = useMemo(() => mermaidSource({...specification,direction}), [specification, direction]);
  const [svg, setSvg] = useState<string>("");
  const [failed, setFailed] = useState(false);
  const [zoom, setZoom] = useState(1);
  const viewport = useRef<HTMLDivElement>(null);
  const shell = useRef<HTMLDivElement>(null);
  const drag = useRef<{x:number;y:number;left:number;top:number} | null>(null);
  const [size, setSize] = useState({width:800,height:400});
  const fit = () => {
    const element = viewport.current;
    if (element) { setZoom(Math.max(.08, Math.min(1.5, (element.clientWidth - 36) / size.width, (element.clientHeight - 36) / size.height))); element.scrollTo?.(0, 0); }
  };
  useEffect(() => {
    const element = viewport.current;
    if (element) { setZoom(Math.max(.8, Math.min(1, (element.clientWidth - 36) / size.width, (element.clientHeight - 36) / size.height))); element.scrollTo?.(0, 0); }
  }, [size]);
  const [dark, setDark] = useState(() => window.matchMedia?.("(prefers-color-scheme: dark)").matches ?? false);

  useEffect(() => {
    const media = window.matchMedia?.("(prefers-color-scheme: dark)");
    if (!media) return;
    const update = () => setDark(media.matches);
    media.addEventListener?.("change", update);
    return () => media.removeEventListener?.("change", update);
  }, []);

  useEffect(() => {
    let active = true;
    setFailed(false);
    import("mermaid")
      .then(async ({ default: mermaid }) => {
        mermaid.initialize({
          startOnLoad: false,
          securityLevel: "strict",
          htmlLabels: false,
          theme: "base",
          themeVariables: {
            background: dark ? "#121916" : "#F7FAF8",
            primaryTextColor: dark ? "#F1F6F2" : "#172019",
            lineColor: dark ? "#9BAFA2" : "#58675D",
            edgeLabelBackground: dark ? "#1D2821" : "#F7FAF8",
            fontFamily: "Inter, ui-sans-serif, system-ui, sans-serif",
          },
          flowchart: { curve: "basis", htmlLabels: false, useMaxWidth: false, nodeSpacing: 48, rankSpacing: 64, padding: 18 },
        });
        const result = await mermaid.render(diagramId, source);
        if (active) {
          const document = new DOMParser().parseFromString(result.svg, "image/svg+xml");
          const box = document.documentElement.getAttribute("viewBox")?.split(/[ ,]+/).map(Number);
          setSvg(result.svg);
          if (box?.length === 4) setSize({width:box[2],height:box[3]});
        }
      })
      .catch(() => active && setFailed(true));
    return () => { active = false; };
  }, [dark, diagramId, source]);

  return (
    <div className="mermaid-shell" ref={shell}>
      {svg && !failed ? (
        <>
          <div className="diagram-toolbar" aria-label="Diagram view controls">
            <div><strong>Reasoning map</strong><span>Drag to explore · zoom for detail</span></div>
            <div className="diagram-actions">
              <select aria-label="Diagram direction" value={direction} onChange={event => setDirection(event.target.value as MermaidSpecification["direction"])}><option value="top_down">Vertical</option><option value="left_right">Horizontal</option></select>
              <button type="button" onClick={() => setZoom((value) => Math.max(.08, value / 1.25))} aria-label="Zoom diagram out">−</button>
              <output aria-live="polite">{Math.round(zoom * 100)}%</output>
              <button type="button" onClick={() => setZoom((value) => Math.min(3, value * 1.25))} aria-label="Zoom diagram in">+</button>
              <button type="button" onClick={fit}>Fit</button>
              <button type="button" onClick={() => { void (document.fullscreenElement ? document.exitFullscreen() : shell.current?.requestFullscreen())?.catch(() => undefined); }}>Fullscreen</button>
            </div>
          </div>
          <div className="mermaid-viewport" ref={viewport} tabIndex={0} onPointerDown={event => { if(event.button !== 0) return; drag.current = {x:event.clientX,y:event.clientY,left:event.currentTarget.scrollLeft,top:event.currentTarget.scrollTop}; event.currentTarget.setPointerCapture(event.pointerId); }} onPointerMove={event => { const start = drag.current; if(start) { event.currentTarget.scrollLeft = start.left - event.clientX + start.x; event.currentTarget.scrollTop = start.top - event.clientY + start.y; } }} onPointerUp={() => { drag.current = null; }} onPointerCancel={() => { drag.current = null; }}>
            <div className="mermaid-canvas" role="img" aria-label={specification.textual_alternative} style={{ width:size.width * zoom, height:size.height * zoom }} dangerouslySetInnerHTML={{ __html: svg }} />
          </div>
          <div className="diagram-legend" aria-label="Diagram color legend"><span className="research">Research</span><span className="development">Development</span><span className="decision">Decision</span><span className="continuity">Continuity</span><span className="feedback">Feedback</span></div>
        </>
      ) : failed ? (
        <p role="alert">Diagram rendering unavailable.</p>
      ) : (
        <p aria-live="polite">Preparing diagram…</p>
      )}
      <details><summary>Text alternative</summary><p>{specification.textual_alternative}</p></details>
    </div>
  );
}
