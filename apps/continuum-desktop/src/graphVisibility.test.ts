import { describe, expect, it } from "vitest";
import type { Edge, Node } from "@xyflow/react";
import { deriveVisibleGraph } from "./graphVisibility";

function node(id: string, label = id): Node {
  return { id, position: { x: 0, y: 0 }, data: { label, kind: "fixture" } };
}

function edge(source: string, target: string): Edge {
  return { id: `${source}-${target}`, source, target };
}

describe("deriveVisibleGraph", () => {
  const graph = {
    nodes: [node("a", "Research"), node("b", "Finding"), node("c", "Decision"), node("d", "Development")],
    edges: [edge("a", "b"), edge("b", "c"), edge("a", "d")],
  };

  it("collapses only the selected branch descendants", () => {
    const visible = deriveVisibleGraph(graph, new Set(["b"]), "");
    expect(visible.nodes.map((item) => item.id)).toEqual(["a", "b", "d"]);
    expect(visible.edges.map((item) => item.id)).toEqual(["a-b", "a-d"]);
  });

  it("applies search after branch visibility without resurfacing hidden nodes", () => {
    expect(deriveVisibleGraph(graph, new Set(["b"]), "Decision").nodes).toEqual([]);
    expect(deriveVisibleGraph(graph, new Set(), "Development").nodes.map((item) => item.id)).toEqual(["d"]);
  });

  it("terminates safely for cycles and keeps the collapsed root visible", () => {
    const cyclic = { nodes: [node("a"), node("b")], edges: [edge("a", "b"), edge("b", "a")] };
    expect(deriveVisibleGraph(cyclic, new Set(["a"]), "").nodes.map((item) => item.id)).toEqual(["a"]);
  });
});
