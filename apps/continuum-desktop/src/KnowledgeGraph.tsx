import { useCallback, useEffect, useMemo, useState } from "react";
import {
  Background,
  Controls,
  MiniMap,
  ReactFlow,
  applyNodeChanges,
  Position,
  type Edge,
  type Node,
  type NodeMouseHandler,
} from "@xyflow/react";
import ELK from "elkjs/lib/elk.bundled.js";
import "@xyflow/react/dist/style.css";
import type { DiagramNode, GraphSpecification } from "./contracts";
import { deriveVisibleGraph, type LaidOutGraph } from "./graphVisibility";

const elk = new ELK();
const nodeWidth = 250;
const nodeHeight = 124;

function GraphNodeCard({ node }: { node: DiagramNode }) {
  return <div className="graph-node-card">
    {node.preview_data_uri && <img className="graph-evidence-preview" src={node.preview_data_uri} alt={`Evidence: ${node.label}`} />}
    <small>{node.kind.replaceAll("_", " ")} · {node.status.replaceAll("_", " ")}</small>
    <strong>{node.label}</strong>
  </div>;
}

function nodeTone(kind: string, status: string): string {
  const value = `${kind} ${status}`.toLowerCase();
  if (/(failed|blocked|rejected)/.test(value)) return "attention";
  if (/(checkpoint|context|continuity)/.test(value)) return "continuity";
  if (/(decision|requirement)/.test(value)) return "decision";
  if (/(change|code|repository|commit|test|development)/.test(value)) return "development";
  if (/(feedback|learning)/.test(value)) return "feedback";
  if (/(question|evidence|experiment|result|finding|research)/.test(value)) return "research";
  return "neutral";
}

async function layout(specification: GraphSpecification): Promise<LaidOutGraph> {
  const result = await elk.layout({
    id: "root",
    layoutOptions: {
      "elk.algorithm": "layered",
      "elk.direction": specification.direction === "top_down" ? "DOWN" : "RIGHT",
      "elk.spacing.nodeNode": "38",
      "elk.layered.spacing.nodeNodeBetweenLayers": "72",
    },
    children: specification.nodes.map((node) => ({ id: node.id, width: nodeWidth, height: nodeHeight })),
    edges: specification.edges.map((edge) => ({ id: edge.id, sources: [edge.source], targets: [edge.target] })),
  });
  const byId = new Map(specification.nodes.map((node) => [node.id, node]));
  return {
    nodes: (result.children ?? []).map((item) => {
      const source = byId.get(item.id)!;
      return {
        id: source.id,
        position: { x: item.x ?? 0, y: item.y ?? 0 },
        data: { label: <GraphNodeCard node={source} />, searchLabel: source.label, kind: source.kind, status: source.status },
        sourcePosition: specification.direction === "top_down" ? Position.Bottom : Position.Right,
        targetPosition: specification.direction === "top_down" ? Position.Top : Position.Left,
        className: `graph-node tone-${nodeTone(source.kind, source.status)}`,
        style: { width: nodeWidth, height: nodeHeight },
        ariaLabel: `${source.kind}: ${source.label}; status ${source.status}`,
      } satisfies Node;
    }),
    edges: specification.edges.map((edge) => ({
      id: edge.id,
      source: edge.source,
      target: edge.target,
      label: edge.label,
      type: "smoothstep",
      animated: false,
    })),
  };
}

export function KnowledgeGraph({ specification }: { specification: GraphSpecification }) {
  const [query, setQuery] = useState("");
  const [selected, setSelected] = useState<string | null>(null);
  const [collapsedRoots, setCollapsedRoots] = useState<Set<string>>(() => new Set());
  const [laidOut, setLaidOut] = useState<LaidOutGraph>({ nodes: [], edges: [] });
  const [failed, setFailed] = useState(false);

  useEffect(() => {
    let active = true;
    setFailed(false);
    layout(specification)
      .then((value) => active && setLaidOut(value))
      .catch(() => active && setFailed(true));
    return () => { active = false; };
  }, [specification]);

  const visible = useMemo(
    () => deriveVisibleGraph(laidOut, collapsedRoots, query),
    [collapsedRoots, laidOut, query],
  );
  const selectedNode = useMemo(
    () => specification.nodes.find((node) => node.id === selected) ?? null,
    [selected, specification.nodes],
  );

  const onNodeClick: NodeMouseHandler = useCallback((_event, node) => setSelected(node.id), []);
  const onNodeDoubleClick: NodeMouseHandler = useCallback((_event, node) => {
    setSelected(node.id);
    setCollapsedRoots((current) => {
      const next = new Set(current);
      if (next.has(node.id)) next.delete(node.id);
      else next.add(node.id);
      return next;
    });
  }, []);
  const toggleSelectedBranch = useCallback(() => {
    if (!selected) return;
    setCollapsedRoots((current) => {
      const next = new Set(current);
      if (next.has(selected)) next.delete(selected);
      else next.add(selected);
      return next;
    });
  }, [selected]);

  if (failed) return <p role="alert">Diagram layout unavailable. {specification.textual_alternative}</p>;
  return (
    <div className="graph-shell">
      <div className="graph-intro">
        <strong>Follow the reasoning, not the file list.</strong>
        <span>Questions branch into Evidence, Findings, Decisions, Requirements, and implementation. Double-click a node to focus its branch.</span>
      </div>
      <div className="graph-toolbar">
        <label>Find a concept<input value={query} onChange={(event) => setQuery(event.target.value)} placeholder="Question, finding, decision…" /></label>
        <div className="graph-actions" aria-label="Graph expansion controls">
          <button type="button" disabled={!selected} onClick={toggleSelectedBranch}>
            {selected && collapsedRoots.has(selected) ? "Expand branch" : "Collapse branch"}
          </button>
          <button type="button" disabled={collapsedRoots.size === 0} onClick={() => setCollapsedRoots(new Set())}>Expand all</button>
        </div>
        <span aria-live="polite">{visible.nodes.length} nodes · {visible.edges.length} links</span>
      </div>
      <div className="graph-canvas" role="img" aria-label={specification.textual_alternative}>
        <ReactFlow nodes={visible.nodes} edges={visible.edges} onNodesChange={changes => setLaidOut(current => ({ ...current, nodes: applyNodeChanges(changes, current.nodes) }))} onNodeClick={onNodeClick} onNodeDoubleClick={onNodeDoubleClick} fitView fitViewOptions={{ padding: .2, maxZoom: 1 }} minZoom={0.15} maxZoom={2.5}>
          <Background gap={22} size={1} />
          <MiniMap pannable zoomable ariaLabel="Graph overview" nodeColor="#176b52" nodeStrokeColor="#103b2e" nodeStrokeWidth={2} maskColor="rgba(20, 45, 34, .24)" style={{ background: "#e8f5ef", width: 150, height: 100 }} />
          <Controls showInteractive={false} />
        </ReactFlow>
      </div>
      {selectedNode && <p className="selection" aria-live="polite"><strong>{selectedNode.label}</strong> · {selectedNode.kind.replaceAll("_", " ")} · {selectedNode.status.replaceAll("_", " ")}. Double-click to focus or restore its branch.</p>}
      {specification.truncated && <p className="notice">This graph is scoped to its safety and performance budget.</p>}
      <details><summary>Text alternative</summary><p>{specification.textual_alternative}</p></details>
    </div>
  );
}
