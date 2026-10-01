import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import App from "./App";
import { mermaidSource } from "./MermaidDiagram";

vi.mock("./KnowledgeGraph", () => ({ KnowledgeGraph: () => <div aria-label="interactive knowledge graph" /> }));
vi.mock("./MermaidDiagram", async (importOriginal) => {
  const actual = await importOriginal<typeof import("./MermaidDiagram")>();
  return { ...actual, MermaidDiagram: () => <div aria-label="portable diagram" /> };
});

describe("CP8 human report", () => {
  it("renders the useful state, next actions, timeline, and citations", async () => {
    render(<App />);
    expect(screen.getByRole("heading", { level: 1 })).toHaveTextContent("Connected R&D Overview");
    expect(screen.getByText("Continue CP9 research capture")).toBeInTheDocument();
    expect(screen.getByRole("heading", { name: "How the work evolved" })).toBeInTheDocument();
    expect(screen.getByText("Use durable semantic checkpoints")).toBeInTheDocument();
    expect((await screen.findAllByLabelText("portable diagram")).length).toBeGreaterThan(0);
    const explorer = screen.getByText("Explore individual sources and evidence images").closest("details")!;
    explorer.open = true;
    fireEvent(explorer, new Event("toggle"));
    expect(await screen.findByLabelText("interactive knowledge graph")).toBeInTheDocument();
  });

  it("maps untrusted diagram IDs to safe local aliases", () => {
    const source = mermaidSource({
      diagram_kind: "flowchart",
      direction: "left_right",
      textual_alternative: "safe",
      nodes: [{ id: "click evil call", label: "<script>alert(1)</script>", kind: "concept", status: "active", source_ids: [] }],
      edges: [],
    });
    expect(source).toContain("n0");
    expect(source).toContain("classDef research");
    expect(source).toContain(":::neutral");
    expect(source).not.toContain("click evil call");
    expect(source).not.toContain("<script>");
    expect(source.toLowerCase()).not.toContain("fill:#fff,");
  });
});
