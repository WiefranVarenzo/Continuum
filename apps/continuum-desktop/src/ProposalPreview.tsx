import { lazy, Suspense } from "react";
import type { ExternalProposal } from "./mcpContracts";
import type { MermaidSpecification } from "./contracts";
const Diagram = lazy(() => import("./MermaidDiagram").then(module => ({default: module.MermaidDiagram})));

export function ProposalPreview({ proposal }: { proposal: ExternalProposal }) {
  const payload: Record<string, unknown> = proposal.payload && typeof proposal.payload === "object" && !Array.isArray(proposal.payload) ? proposal.payload as Record<string, unknown> : {};
  const fields: [string,string][] = [
    ["suggested_title", "Suggested title"], ["observed_content", "What the evidence shows"],
    ["suggested_description", "Description"], ["relationship_to_research", "Why it matters to your research"],
    ["claim", "Finding"], ["interpretation", "Explanation"], ["summary", "Research summary"],
    ["uncertainty", "What remains uncertain"], ["next_action", "Next step"],
    ["key_points", "Key findings"], ["limitations", "Limits of the evidence"], ["recommendations", "Recommended next steps"],
  ];
  return <div className="proposal-readable">
    <p className="proposal-draft-label">AI draft · check against your evidence before accepting</p>
    {fields.map(([key,label]) => typeof payload[key] === "string" && payload[key] ? <div key={key}><h4>{label}</h4><p>{String(payload[key])}</p></div> : Array.isArray(payload[key]) ? <div key={key}><h4>{label}</h4><ul>{(payload[key] as unknown[]).filter(item => typeof item === "string").map((item, i) => <li key={i}>{String(item)}</li>)}</ul></div> : null)}
    {proposal.kind === "diagram_plan" && Array.isArray(payload.nodes) && Array.isArray(payload.edges) && <Suspense fallback={<p>Preparing proposed diagram…</p>}><Diagram specification={{...payload, diagram_kind:"flowchart"} as unknown as MermaidSpecification} /></Suspense>}
  </div>;
}
