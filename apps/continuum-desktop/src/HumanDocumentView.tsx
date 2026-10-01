import { Suspense, lazy, useState } from "react";
import type { BlockContent, Citation, HumanDocument, HumanDocumentBlock } from "./contracts";
import "./report.css";

const KnowledgeGraph = lazy(() => import("./KnowledgeGraph").then((module) => ({ default: module.KnowledgeGraph })));
const MermaidDiagram = lazy(() => import("./MermaidDiagram").then((module) => ({ default: module.MermaidDiagram })));

function sourceAnchor(id: string) {
  return `source-${id.replace(/[^A-Za-z0-9_-]/g, "-")}`;
}

function Sources({ ids }: { ids: string[] }) {
  if (!ids.length) return null;
  return <details className="source-links"><summary>View {ids.length} verified source reference{ids.length === 1 ? "" : "s"}</summary><p>{ids.map((id, index) => <span key={id}>{index > 0 && ", "}<a href={`#${sourceAnchor(id)}`}>Source {index + 1}</a></span>)}</p></details>;
}

const friendlyTitles: Record<string, string> = {
  overview: "Executive summary",
  status: "Project at a glance",
  "knowledge-graph": "Research & project knowledge map",
  "portable-flow": "Evidence-to-decision flow",
  timeline: "How the work evolved",
  "table-research_question": "Research questions",
  "table-evidence": "Evidence collected",
  "table-experiment": "Experiments",
  "table-result": "Observed results",
  "table-finding": "Findings",
  "table-decision": "Decisions and rationale",
  "table-requirement": "Requirements",
  "table-change_set": "Implementation changes",
  "table-repository": "Connected repositories",
  "table-code_entity": "Technical components",
  citations: "Sources and provenance",
};

function blockTitle(block: HumanDocumentBlock): string {
  return friendlyTitles[block.meta.id] || block.meta.title;
}

function contributionLabel(kind: HumanDocumentBlock["meta"]["contribution"]["kind"]): string {
  return kind === "deterministic" ? "Verified project data"
    : kind === "ai_assisted" ? "AI-assisted · reviewed"
      : kind === "user_authored" ? "Written by the user"
        : "Imported source";
}

function List({ title, values }: { title: string; values: string[] }) {
  if (!values.length) return null;
  return <div><h3>{title}</h3><ul>{values.map((value) => <li key={value}>{value}</li>)}</ul></div>;
}

function Citations({ ids, citations }: { ids: string[]; citations: Citation[] }) {
  const wanted = new Set(ids);
  return <ol className="citations">{citations.filter((citation) => wanted.has(citation.source_id)).map((citation) => (
    <li id={sourceAnchor(citation.source_id)} key={citation.source_id}>
      <strong>{citation.title}</strong>
      <span>{citation.source_type} · {citation.status} · v{citation.version} · {citation.classification}</span>
      <details><summary>Technical reference</summary><code>{citation.source_id}</code></details>
    </li>
  ))}</ol>;
}

function SourceExplorer({ content }: { content: Extract<BlockContent, {type: "graph"}> }) {
  const [open, setOpen] = useState(false);
  return <details onToggle={event => setOpen(event.currentTarget.open)}><summary>Explore individual sources and evidence images</summary>{open && <Suspense fallback={<p>Preparing source explorer…</p>}><KnowledgeGraph specification={content.specification} /></Suspense>}</details>;
}

function Content({ content, citations }: { content: BlockContent; citations: Citation[] }) {
  switch (content.type) {
    case "overview": return <><p className="lead">{content.summary}</p><p>Status: <strong>{content.status}</strong></p><div className="overview-grid"><List title="Blockers" values={content.blockers} /><List title="Next actions" values={content.next_actions} /></div></>;
    case "status_cards": return <div className="cards">{content.cards.map((card) => <article className={`card state-${card.state}`} key={`${card.label}-${card.value}`}><span>{card.label}</span><strong>{card.value}</strong></article>)}</div>;
    case "heading": return <p>Heading level {content.level} marker.</p>;
    case "prose": return <p>{content.text}</p>;
    case "callout": return <aside className={`callout ${content.tone}`}>{content.text}</aside>;
    case "research_synthesis": return <div className="research-synthesis"><p className="lead">{content.summary}</p><div className="synthesis-grid"><List title="What the evidence shows" values={content.key_points} /><List title="Recommended next steps" values={content.recommendations} /></div>{content.limitations.length > 0 && <aside className="callout uncertainty"><strong>What still needs verification</strong><ul>{content.limitations.map(item => <li key={item}>{item}</li>)}</ul></aside>}</div>;
    case "key_value": return <dl>{content.facts.map((fact) => <div key={fact.key}><dt>{fact.key}</dt><dd>{fact.value}</dd></div>)}</dl>;
    case "table": return <div className="table-wrap" tabIndex={0} role="region" aria-label="Scrollable data table"><table><thead><tr>{content.columns.map((column) => <th scope="col" data-column={column.key} key={column.key}>{column.label}</th>)}</tr></thead><tbody>{content.rows.map((row) => <tr key={row.id}>{content.columns.map((column, index) => index === 0
      ? <th scope="row" data-column={column.key} key={column.key}>{row.cells[column.key] ?? "—"}</th>
      : <td data-column={column.key} key={column.key}>{row.cells[column.key] ?? "—"}</td>)}</tr>)}</tbody></table>{content.truncated && <p className="notice">More records exist outside this bounded view.</p>}</div>;
    case "timeline": return <ol className="timeline">{content.entries.map((entry) => <li key={entry.id}><time dateTime={entry.occurred_at}>{new Date(entry.occurred_at).toLocaleString()}</time><strong>{entry.label}</strong><span>{entry.detail}</span></li>)}</ol>;
    case "citation_list": return <Citations ids={content.citation_ids} citations={citations} />;
    case "artifact_reference": return <div><p>{content.description}</p><dl><div><dt>Artifact</dt><dd><code>{content.artifact_id}</code></dd></div><div><dt>Type</dt><dd>{content.media_type}</dd></div><div><dt>Availability</dt><dd>{content.availability}</dd></div></dl></div>;
    case "graph": return <><Suspense fallback={<p aria-live="polite">Preparing research map…</p>}><MermaidDiagram specification={{...content.specification, diagram_kind:"flowchart"}} /></Suspense><SourceExplorer content={content} /></>;
    case "mermaid_diagram": return <Suspense fallback={<p aria-live="polite">Preparing diagram…</p>}><MermaidDiagram specification={content.specification} /></Suspense>;
    case "detail_group": return <details><summary>{content.summary}</summary><ul>{content.details.map((detail) => <li key={detail}>{detail}</li>)}</ul></details>;
  }
}

function Block({ block, citations }: { block: HumanDocumentBlock; citations: Citation[] }) {
  const title = blockTitle(block);
  return (
    <section id={block.meta.id} data-block-type={block.content.type} aria-labelledby={`${block.meta.id}-title`}>
      <div className="section-heading"><h2 id={`${block.meta.id}-title`}>{title}</h2><span className="badge">{contributionLabel(block.meta.contribution.kind)}</span></div>
      <Content content={block.content} citations={citations} />
      <Sources ids={block.meta.source_ids} />
    </section>
  );
}

export function HumanDocumentView({ document }: { document: HumanDocument }) {
  const visibleBlocks = document.blocks.filter((block) => block.content.type !== "heading");
  return <>
    <a className="skip" href="#main">Skip to report</a>
    <header className="report-header">
      <p className="eyebrow">CONTINUUM PROJECT REPORT</p>
      <h1>{document.title}</h1>
      <p className="report-deck">A readable explanation of what was researched, what the evidence supports, what was decided, and how the project changed.</p>
      <p className="meta">Updated {new Date(document.generated_at).toLocaleString()} · {document.classification} project knowledge</p>
      <details className="report-integrity"><summary>Verification details</summary><p>Ledger position {document.source_ledger_sequence}. This report is a readable projection; canonical project records remain authoritative.</p></details>
    </header>
    <main id="main">
      <nav className="report-toc" aria-label="Report contents"><strong>In this report</strong><div>{visibleBlocks.map((block) => <a key={block.meta.id} href={`#${block.meta.id}`}>{blockTitle(block)}</a>)}</div></nav>
      {(document.omissions.length > 0 || document.limitations.length > 0) && <details className="report-scope"><summary>Scope notes and limitations ({document.omissions.length + document.limitations.length})</summary><ul>{[...document.omissions, ...document.limitations].map((item) => <li key={item}>{item}</li>)}</ul></details>}
      {visibleBlocks.map((block) => <Block key={block.meta.id} block={block} citations={document.citations} />)}
    </main>
    <footer><details><summary>Technical integrity</summary><p>Material fingerprint <code>{document.material_fingerprint}</code></p><p>Human Document schema v{document.schema_version}</p></details></footer>
  </>;
}
