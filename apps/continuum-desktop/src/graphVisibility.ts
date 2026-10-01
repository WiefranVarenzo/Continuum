import type { Edge, Node } from "@xyflow/react";

export type LaidOutGraph = { nodes: Node[]; edges: Edge[] };

export function deriveVisibleGraph(
  laidOut: LaidOutGraph,
  collapsedRoots: ReadonlySet<string>,
  query: string,
): LaidOutGraph {
  const outgoing = new Map<string, string[]>();
  for (const edge of laidOut.edges) {
    const targets = outgoing.get(edge.source) ?? [];
    targets.push(edge.target);
    outgoing.set(edge.source, targets);
  }

  const hidden = new Set<string>();
  for (const root of collapsedRoots) {
    const pending = [...(outgoing.get(root) ?? [])];
    while (pending.length > 0) {
      const candidate = pending.pop()!;
      if (candidate === root || hidden.has(candidate)) continue;
      hidden.add(candidate);
      pending.push(...(outgoing.get(candidate) ?? []));
    }
  }

  const expandedNodes = laidOut.nodes.filter((node) => !hidden.has(node.id));
  const expandedIds = new Set(expandedNodes.map((node) => node.id));
  const expandedEdges = laidOut.edges.filter(
    (edge) => expandedIds.has(edge.source) && expandedIds.has(edge.target),
  );
  const needle = query.trim().toLocaleLowerCase();
  if (!needle) return { nodes: expandedNodes, edges: expandedEdges };

  const matchingIds = new Set(
    expandedNodes
      .filter((node) =>
        `${String(node.data.searchLabel ?? node.data.label)} ${String(node.data.kind)}`
          .toLocaleLowerCase()
          .includes(needle),
      )
      .map((node) => node.id),
  );
  return {
    nodes: expandedNodes.filter((node) => matchingIds.has(node.id)),
    edges: expandedEdges.filter(
      (edge) => matchingIds.has(edge.source) && matchingIds.has(edge.target),
    ),
  };
}
