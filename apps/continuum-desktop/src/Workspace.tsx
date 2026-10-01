import { useCallback, useEffect, useMemo, useRef, useState, type FormEvent, type ReactNode } from "react";
import { captureExternalEvidence, createCheckpoint, getCurrentProjectState } from "./bridge";
import { archiveEvidence, clipboardPng, detectClients, exportContextMarkdown, imageFingerprint, researchItems, runAiOrganizer, setCaptureDock, startResearch, updateEvidence, updateResearchQuestion, updateResearchSession, type ClientDetection, type WorkspaceItem } from "./workspaceBridge";
import { ResearchBoard, EvidenceImage } from "./ResearchBoard";
import { KnowledgeGraph } from "./KnowledgeGraph";
import { CapturePanel } from "./CapturePanel";
import { ContinuityPanel } from "./ContinuityPanel";
import { AiContinuityPanel } from "./AiContinuityPanel";
import { ReportStudio } from "./ReportStudio";
import { Messages } from "./Messages";
import { MemoryPanel } from "./MemoryPanel";
import { runAssistant, flushProjectDocuments, registerCloseTask, prepareWorkspaceClose, type AssistantAgent, type AssistantResult } from "./projectDocuments";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { invoke } from "@tauri-apps/api/core";
import { WorkspaceQuickStart } from "./WorkspaceQuickStart";
import type { HumanDocument } from "./contracts";
import type { OpenedProject, RepositorySyncSummary } from "./releaseContracts";
import type { SpaceCapabilities } from "./contextContracts";
import "./workspace.css";
import "./assistantWorkspace.css";
import { listen } from "@tauri-apps/api/event";

type Page = "home" | "board" | "history" | "development" | "capture" | "reports" | "context" | "messages" | "ai" | "settings";
interface Props {
  project: OpenedProject; capabilities: SpaceCapabilities; document: HumanDocument; settings: ReactNode;
  busy: boolean; onRefresh: () => Promise<void>; onExport: (format: "html" | "markdown") => Promise<void>;
  onRepository: (path: string) => Promise<RepositorySyncSummary>;
  onAllProjects: () => Promise<void>;
}
const message = (cause: unknown) => cause instanceof Error ? cause.message : String(cause);
function readSaved(key: string): string { try { return localStorage.getItem(key) || ""; } catch { return ""; } }
const markChanged = () => window.dispatchEvent(new Event("continuum:changed"));

export function Workspace({ project, capabilities, document: report, settings, busy, onRefresh, onExport, onRepository, onAllProjects }: Props) {
  const scope = capabilities.research && capabilities.development ? "integrated" : capabilities.research ? "research" : "development";
  const mode = scope === "integrated" ? "Connected R&D" : scope === "research" ? "Research only" : "Development only";
  const storageKey = `continuum.workspace.${project.project_id}`;
  const [page, setPage] = useState<Page>("home");
  const [sessionId, setSessionId] = useState(() => readSaved(`${storageKey}.session`));
  const [sessions, setSessions] = useState<WorkspaceItem[]>([]);
  const [items, setItems] = useState<WorkspaceItem[]>([]);
  const [nextPage, setNextPage] = useState<number | null>(null);
  const [nextHistory, setNextHistory] = useState<number | null>(null);
  const [search, setSearch] = useState("");
  const [tray, setTray] = useState(true);
  const [newResearch, setNewResearch] = useState(false);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [pendingImages, setPendingImages] = useState(0);
  const [error, setError] = useState("");
  const [notice, setNotice] = useState("");
  const [saveStatus, setSaveStatus] = useState("Restoring workspace…");
  const [live, setLive] = useState(false);
  const [liveStarting, setLiveStarting] = useState(false);
  const [compact, setCompact] = useState(false);
  const [recording, setRecording] = useState(false);
  const [captureStatus, setCaptureStatus] = useState("");
  const [noteEach, setNoteEach] = useState(true);
  const [inspected, setInspected] = useState<WorkspaceItem | null>(null);
  const [note, setNote] = useState("");
  const [editTitle, setEditTitle] = useState("");
  const [clients, setClients] = useState<ClientDetection[]>([]);
  const [preferredClient, setPreferredClient] = useState<"codex" | "claude_code" | "gemini_cli" | undefined>();
  const [agent,setAgent]=useState<AssistantAgent>(()=>readSaved(`${storageKey}.agent`)==="hermes"?"hermes":"codex");
  const [aiDraft,setAiDraft]=useState<AssistantResult|null>(null);
  const [organizing, setOrganizing] = useState(false);
  const [analysisSeconds, setAnalysisSeconds] = useState(0);
  const [exitState,setExitState]=useState<"idle"|"saving"|"blocked">("idle");
  const [exitError,setExitError]=useState("");
  const exitInFlight=useRef(false);
  const attemptExit=useRef(async()=>{});
  useEffect(() => {
    const subscription = listen<{project_id: string; elapsed_seconds: number}>("continuum:assistant-progress", event => {
      if (event.payload.project_id === project.project_id) setAnalysisSeconds(event.payload.elapsed_seconds);
    }).catch(() => () => undefined);
    return () => { void subscription.then(unlisten => unlisten()); };
  }, []);
  const [revision, setRevision] = useState(0);
  const [draft, setDraft] = useState(() => { try { const value = JSON.parse(readSaved(`${storageKey}.draft`)); return value && ["title", "question", "context", "desiredOutcome"].every(key => typeof value[key] === "string") ? value as {title:string;question:string;context:string;desiredOutcome:string} : null; } catch { return null; } });
  const emptyDraft = { title: "", question: "", context: "", desiredOutcome: "" };
  const input = draft || emptyDraft;
  const requestId = useRef(crypto.randomUUID());
  const dirty = useRef(false);
  const working = useRef(false);
  const alive = useRef(true);
  const generation = useRef(0);
  const queue = useRef(Promise.resolve());
  const queuedCount = useRef(0);
  const liveRef = useRef(false);
  const lastFingerprint = useRef("");
  const noteQueue = useRef<WorkspaceItem[]>([]);
  const sessionRef = useRef(sessionId);
  sessionRef.current = sessionId;
  const callbacks = useRef({ onRefresh }); callbacks.current = { onRefresh };
  const observedSequence = useRef(report.source_ledger_sequence);
  observedSequence.current = Math.max(observedSequence.current, report.source_ledger_sequence);
  const session = sessions.find(s => s.entity.id === sessionId) || null;
  const graph = report.blocks.find(b => b.content.type === "graph")?.content;
  const activeItems = useMemo(()=>items.filter(i => i.entity.status !== "archived"),[items]);
  const developmentItems=useMemo<WorkspaceItem[]>(()=>graph?.type==="graph"?graph.specification.nodes.filter(n=>!["research_session","research_question","evidence"].includes(n.kind)).map(n=>({entity:{id:n.id,title:n.label,entity_type:n.kind,status:n.status,version:0,updated_at:report.generated_at,metadata:{}},details:{summary:"Development source. Inspect the repository or original project record for authoritative details."}})):[],[graph,report.generated_at]);
  const visible = activeItems.filter(i => `${i.entity.title} ${i.details.annotation || ""}`.toLowerCase().includes(search.toLowerCase()));
  const photos = activeItems.filter(i => i.entity.entity_type === "evidence");

  const loadItems = useCallback(async () => {
    if (!capabilities.research) return;
    const ticket = ++generation.current;
    const [history, sources] = await Promise.all([researchItems(null, true), researchItems(sessionRef.current || null)]);
    if (!alive.current || ticket !== generation.current) return;
    setSessions(history.items); setNextHistory(history.next_offset);
    setItems(sources.items.filter(i => i.entity.entity_type !== "research_session")); setNextPage(sources.next_offset);
    if (sessionRef.current && !history.items.some(s => s.entity.id === sessionRef.current)) {
      // A saved session may be on a later page; do not silently assign its sources to another session.
      setNotice("Selected research is preserved. Load more history if its title is outside the first page.");
    }
  }, [capabilities.research]);

  useEffect(() => {
    alive.current = true;
    void loadItems().then(() => { if (alive.current) { setLoading(false); setSaveStatus("Saved locally"); } }).catch(cause => { if (alive.current) { setError(message(cause)); setLoading(false); } });
    void detectClients().then(result => { if (alive.current) setClients(result); }).catch(cause => { if (alive.current) setNotice(`AI detection unavailable: ${message(cause)}`); });
    return () => { alive.current = false; liveRef.current = false; };
  }, [loadItems]);

  useEffect(() => {
    const changed = () => { dirty.current = true; setSaveStatus("Saving resume point…"); };
    window.addEventListener("continuum:changed", changed);
    const timer = window.setInterval(async () => {
      if (!dirty.current || working.current) return;
      dirty.current = false; working.current = true;
      try {
        await createCheckpoint({ scope, trigger: "policy", note: `Automatic resume point${sessionRef.current ? ` · research session ${sessionRef.current}` : ""}`, blockers: [], risks: [], next_actions: [], semantic_candidate_id: null, supersedes_checkpoint_id: null });
        await Promise.all([loadItems(), callbacks.current.onRefresh()]);
        if (alive.current) { setSaveStatus("Saved locally · resume point updated"); setRevision(n => n + 1); }
      } catch (cause) { if (alive.current) { setSaveStatus("Source saved; resume refresh needs attention"); setError(message(cause)); } dirty.current = true; }
      finally { working.current = false; }
    }, 2000);
    return () => { clearInterval(timer); window.removeEventListener("continuum:changed", changed); };
  }, [scope, loadItems]);

  useEffect(() => {
    let pending = false;
    const timer = window.setInterval(async () => {
      if (pending || working.current) return;
      pending = true;
      try {
        const current = await getCurrentProjectState(scope);
        if (alive.current && current.as_of_ledger_sequence > observedSequence.current) {
          observedSequence.current = current.as_of_ledger_sequence;
          markChanged();
        }
      } catch (cause) { if (alive.current) setError(`Could not refresh external changes: ${message(cause)}`); }
      finally { pending = false; }
    }, 15000);
    return () => clearInterval(timer);
  }, [scope]);

  useEffect(() => {
    void loadItems().catch(cause => setError(message(cause)));
    try { localStorage.setItem(`${storageKey}.session`, sessionId); } catch { setNotice("Workspace preferences could not be saved; project records remain saved."); }
  }, [sessionId, storageKey, loadItems]);

  async function start(event: FormEvent) {
    event.preventDefault(); if (saving || !input.title.trim() || !input.question.trim()) return;
    setSaving(true); setError("");
    try {
      const created = await startResearch({ ...input, requestId: requestId.current });
      setSessions(current => [created, ...current.filter(s => s.entity.id !== created.entity.id)]);
      sessionRef.current = created.entity.id; setSessionId(created.entity.id); setNewResearch(false); setPage("board");
      setDraft(null); requestId.current = crypto.randomUUID();
      try { localStorage.removeItem(`${storageKey}.draft`); } catch { /* The research is already durable in the project. */ }
      markChanged(); setNotice("Research started. Paste a screenshot here or enable Live screenshots.");
    } catch (cause) { setError(message(cause)); } finally { setSaving(false); }
  }
  function updateDraft(field: keyof typeof emptyDraft, value: string) {
    const next = { ...input, [field]: value }; setDraft(next);
    try { localStorage.setItem(`${storageKey}.draft`, JSON.stringify(next)); } catch { setError("Could not save the draft on this device."); }
  }
  const inspect = useCallback((item: WorkspaceItem) => {
    setInspected(item);
    setEditTitle(item.entity.title);
    setNote(String(item.details.annotation || item.details.question || item.details.objective || ""));
  }, []);

  async function saveImage(blob: Blob, title: string, targetSession: string) {
    if (!targetSession) throw new Error("Start or select a research session before adding screenshots.");
    if (!["image/png","image/jpeg","image/webp","video/mp4","video/webm","audio/mpeg","audio/wav","audio/ogg","audio/webm"].includes(blob.type)) throw new Error("Use PNG/JPEG/WebP images, MP4/WebM video, or MP3/WAV/OGG/WebM audio.");
    if (blob.size > 32 * 1024 * 1024) throw new Error("Screenshot exceeds the 32 MB limit.");
    const item = await captureExternalEvidence({ kind: blob.type.startsWith("image/") ? "screenshot" : "file", title, source_uri: null, source_title: title,
      captured_at: new Date().toISOString(), media_type: blob.type, bytes: Array.from(new Uint8Array(await blob.arrayBuffer())), source_content: null,
      annotation: "", summary: "Awaiting AI analysis and human review.", relevance: "Evidence collected for this research session.",
      research_session_id: targetSession, research_question_id: null, capture_session_id: null, classification: "internal", metadata: { explicit_user_action: true, capture_method: "workspace_image" } });
    if (alive.current && sessionRef.current === targetSession) {
      const mediaItem = {...(item as WorkspaceItem),details:{...item.details,media_type:blob.type}};
      setItems(current => [mediaItem, ...current]);
      setNotice("Evidence image saved. Give it a meaningful title so AI can relate it correctly.");
      if (noteEach) { noteQueue.current.push(mediaItem); if (noteQueue.current.length === 1) inspect(mediaItem); }
    }
  }
  function enqueueImage(blob: Blob, title = `New evidence · ${new Date().toLocaleTimeString()}`) {
    if (queuedCount.current >= 8) { setError("Eight screenshots are saving. Wait for them to finish before adding more."); return; }
    const targetSession = sessionRef.current;
    queuedCount.current++; setPendingImages(queuedCount.current);
    queue.current = queue.current.then(() => saveImage(blob, title, targetSession)).catch(cause => { if (alive.current) setError(message(cause)); }).finally(() => { queuedCount.current--; if (alive.current) setPendingImages(queuedCount.current); });
  }
  const enqueueRef = useRef(enqueueImage); enqueueRef.current = enqueueImage;
  useEffect(() => {
    if (!sessionId || !capabilities.research) return;
    const paste = (event: ClipboardEvent) => {
      const files = Array.from(event.clipboardData?.files || []).filter(f => /^(image|video|audio)\//.test(f.type));
      if (!files.length) return;
      event.preventDefault(); files.slice(0, 8).forEach(file => enqueueRef.current(file));
    };
    window.addEventListener("paste", paste);
    return () => window.removeEventListener("paste", paste);
  }, [sessionId, capabilities.research]);
  useEffect(() => {
    if (!live) return;
    let pending = false;
    const timer = window.setInterval(async () => {
      if (pending || !liveRef.current || queuedCount.current >= 8) return;
      pending = true;
      try {
        const blob = await clipboardPng();
        if (!blob || !liveRef.current) return;
        const fingerprint = await imageFingerprint(blob);
        if (fingerprint === lastFingerprint.current || !liveRef.current) return;
        lastFingerprint.current = fingerprint; enqueueRef.current(blob);
      } catch (cause) { setError(`Clipboard capture paused: ${message(cause)}. You can still paste or drop screenshots.`); liveRef.current = false; setLive(false); }
      finally { pending = false; }
    }, 1200);
    return () => clearInterval(timer);
  }, [live]);
  async function toggleLive() {
    if (live) { liveRef.current = false; setLive(false); setNotice("Live clipboard capture stopped."); return; }
    setLiveStarting(true);
    try {
      const baseline = await clipboardPng(); lastFingerprint.current = baseline ? await imageFingerprint(baseline) : "";
      liveRef.current = true; setLive(true); setNotice("Live: new clipboard images are saved to this research. Use your system screenshot shortcut and copy the result.");
    } catch (cause) { setError(`Live capture could not start: ${message(cause)}`); }
    finally { setLiveStarting(false); }
  }
  async function toggleDock() {
    try { await setCaptureDock(!compact); setCompact(!compact); } catch (cause) { setError(message(cause)); }
  }
  async function pasteClipboard() {
    try { const blob = await clipboardPng(); if (!blob) throw new Error("No image in the clipboard. Take a screenshot and copy it first."); enqueueImage(blob); } catch (cause) { setError(message(cause)); }
  }
  function dismissNote() { if(noteQueue.current[0]?.entity.id===inspected?.entity.id) noteQueue.current.shift(); const next = noteQueue.current[0]; if (next) inspect(next); else setInspected(null); }
  async function saveNote() {
    if (!inspected) return;
    setSaving(true);
    try {
      let updated: WorkspaceItem;
      if (inspected.entity.entity_type === "research_session") {
        updated = await updateResearchSession(inspected, editTitle.trim(), note.trim());
        setSessions(current => current.map(item => item.entity.id === updated.entity.id ? updated : item));
      } else if (inspected.entity.entity_type === "research_question") {
        updated = await updateResearchQuestion(inspected, editTitle.trim(), note.trim());
        setItems(current => current.map(item => item.entity.id === updated.entity.id ? updated : item));
      } else {
        updated = await updateEvidence(inspected, editTitle.trim(), note.trim());
        setItems(current => current.map(item => item.entity.id === updated.entity.id ? updated : item));
      }
      dismissNote(); markChanged();
    } catch (cause) { setError(message(cause)); } finally { setSaving(false); }
  }
  async function remove(item: WorkspaceItem) {
    try { await archiveEvidence(item); setItems(current => current.filter(i => i.entity.id !== item.entity.id)); markChanged(); setNotice("Removed from the board. The original remains in project history."); } catch (cause) { setError(message(cause)); }
  }
  async function exportMemory() {
    setSaving(true);
    try {
      const path = await exportContextMarkdown({ task: session?.details.objective ? String(session.details.objective) : `Resume ${project.name}`, audience: "local_user", consumer_target: "human-markdown", scope, checkpoint_id: null, freshness_requirement: "current", retrieval_profile: "resume", root_entity_ids: sessionId ? [sessionId] : [], exclude_source_ids: [], include_artifact_content: false,
        budget: { soft_tokens: 6000, hard_tokens: 8000, max_bytes: 131072, max_items: 100, max_item_bytes: 16384 } });
      setNotice(`Context Pack Markdown saved: ${path}`);
    } catch (cause) { setError(message(cause)); } finally { setSaving(false); }
  }
  async function requestOrganization() {
    if (organizing) return;
    if (!window.confirm(`Let ${agent==="hermes"?"Hermes":"Codex"} analyze the selected context and up to ${agent==="hermes"?"1":"8"} image previews? The selected AI provider receives this data. Connections remain suggestions and can be undone.`)) return;
    setOrganizing(true); setAnalysisSeconds(0); setError("");
    try {
      await flushProjectDocuments();
      const result=await runAssistant(project.project_id,page==="development"?"development":scope,page==="development"?null:sessionId||null,agent,"board","Analyze this research/development workspace. Explain each relevant source and propose useful relationships, with reasons. Respect user titles, descriptions and bookmark. Do not invent results.");
      setAiDraft(result);
      setNotice(`Analysis completed: ${result.output.notes.length} draft descriptions, ${result.output.connections.length} suggested connections; ${result.image_count} images inspected. Dashed lines are suggestions, not verified facts. Undo restores the previous board.`);
    } catch(cause){setError(message(cause));} finally {setOrganizing(false);}
  }
  async function saveBookmark() {
    await queue.current;await flushProjectDocuments();
    if(inspected)throw new Error("Save or skip the open evidence description before saving a bookmark.");
    await createCheckpoint({scope,trigger:"manual",note:`Resume ${session?.entity.title||project.name}`,blockers:[],risks:[],next_actions:[],semantic_candidate_id:null,supersedes_checkpoint_id:null});
    setSaveStatus("Saved locally · bookmark updated");setNotice("Bookmark saved. Your sources, board, report edits and messages remain in this project.");
  }
  const closeRef=useRef(async()=>{});
  closeRef.current=async()=>{
    if(recording||live)throw new Error("Stop the live capture before closing.");
    if(organizing)throw new Error("Wait for the AI analysis to finish before closing.");
    await saveBookmark();
  };
  attemptExit.current=async()=>{
    if(exitInFlight.current)return;
    exitInFlight.current=true;setExitState("saving");setExitError("");
    try{
      await prepareWorkspaceClose();
      // close() emits another close-request; destroy() is used only after
      // project writes and the exit bookmark have completed successfully.
      await getCurrentWindow().destroy();
      setExitState("idle");
    }catch(cause){setExitError(message(cause));setExitState("blocked");}
    finally{exitInFlight.current=false;}
  };
  useEffect(()=>registerCloseTask(()=>closeRef.current()),[]);
  useEffect(()=>{
    const sub=getCurrentWindow().onCloseRequested(event=>{
      event.preventDefault();
      void attemptExit.current();
    }).catch(()=>()=>undefined);
    return()=>{void sub.then(stop=>stop());};
  },[]);
  async function exitWithoutSaving(){
    if(!window.confirm("Close Continuum without saving the latest bookmark or pending edits? An active recording or AI request may be lost."))return;
    setExitState("saving");
    try{await getCurrentWindow().destroy();}
    catch(cause){setExitError(message(cause));setExitState("blocked");}
  }
  useEffect(()=>{
    if(!compact)return;
    // Keep the capture window floating, but allow the two-field prompt to fit.
    void import("@tauri-apps/api/dpi").then(({LogicalSize})=>getCurrentWindow().setSize(new LogicalSize(620,inspected?700:220))).catch(()=>undefined);
  },[compact,!!inspected]);
  async function resume(item: WorkspaceItem) {
    if (recording || live || queuedCount.current) { setError("Stop live capture and wait for pending screenshots before switching research."); return; }
    sessionRef.current = item.entity.id; setSessionId(item.entity.id); setPage("board"); setNewResearch(false); markChanged();
  }
  const blockedNavigation = recording || live || pendingImages > 0 || saving || organizing;
  const pages: [Page, string][] = [["home", "Home"], ["board", "Workspace"], ...(capabilities.research ? [["history", "Research history"], ["capture", "Record & sources"]] as [Page,string][] : []), ...(capabilities.development ? [["development", "Development"]] as [Page,string][] : []), ["reports", "Reports"], ["messages", "Messages"], ["ai", "AI connections"], ["settings", "Project settings"]];

  return <div className={`workspace-shell ${compact ? "is-compact" : ""}`}>
    <aside className="workspace-sidebar"><div className="workspace-brand">C<span>CONTINUUM</span></div><strong className="project-name">{project.name}</strong><span className="workspace-mode">{mode}</span>
      {capabilities.research && <button className="new-research" disabled={blockedNavigation} onClick={() => { setNewResearch(true); setPage("board"); }}>＋ New research</button>}
      <nav aria-label="Workspace navigation">{pages.map(([id, label]) => <button key={id} aria-current={page === id ? "page" : undefined} disabled={id === "settings" && blockedNavigation} onClick={() => { setPage(id); setNewResearch(false); }}>{label}</button>)}</nav>
      <div className="sidebar-bottom"><span className="save-dot" />{saveStatus}</div>
    </aside>
    <div className="workspace-main">
      <header className="workspace-topbar"><div><span>{mode} / {page==="context"?"Resume & memory":pages.find(([id]) => page === id)?.[1]}</span><strong>{session?.entity.title || project.name}</strong></div><div className="workspace-tools">{(live || recording) && <span className="live-status">● {recording ? captureStatus || "Preparing capture" : "Live screenshots"}</span>}<button disabled={saving||organizing} onClick={()=>void saveBookmark().catch(c=>setError(message(c)))}>Save bookmark</button><button onClick={()=>setPage("context")}>Resume & memory</button><label className="agent-select">Assistant<select aria-label="Assistant" value={agent} onChange={e=>{const a=e.target.value as AssistantAgent;setAgent(a);try{localStorage.setItem(`${storageKey}.agent`,a);}catch{}}}><option value="codex">Codex</option><option value="hermes">Hermes (optional)</option></select></label><button onClick={() => setPage("ai")}>AI connections{clients.length ? ` · ${clients.length} detected` : ""}</button>{capabilities.research && <button onClick={() => setTray(v => !v)}>Recent captures ({photos.length})</button>}</div></header>
      {error && <div className="workspace-alert" role="alert">{error}<button aria-label="Dismiss error" onClick={() => setError("")}>×</button></div>}
      {notice && <div className="workspace-notice" role="status">{notice}<button aria-label="Dismiss notification" onClick={() => setNotice("")}>×</button></div>}
      {organizing && <div className="analysis-progress" role="status" aria-live="polite"><strong>AI is reading your research · {analysisSeconds}s</strong><p>Using your goal, notes and up to 8 images to prepare evidence descriptions, findings and a report. Suggestions will appear for your review when saved. You can continue reading this workspace.</p></div>}
      {page === "home" && !compact && <section className="workspace-page workspace-home"><div className="page-actions"><div><p className="eyebrow">PROJECT HOME</p><h1>{project.name}</h1><p>Pick up your research or development where you left off.</p></div><button disabled={blockedNavigation} onClick={()=>void onAllProjects().catch(c=>setError(message(c)))}>All projects</button></div><div className="home-actions"><button onClick={()=>setPage("board")}>Open workspace →</button>{capabilities.research&&<button onClick={()=>{setNewResearch(true);setPage("board");}}>Start new research →</button>}{capabilities.development&&<button onClick={()=>setPage("development")}>Open development →</button>}</div>{capabilities.research&&<><h2>Recent research</h2>{sessions.length?<div className="history-grid">{[...sessions].sort((a,b)=>b.entity.updated_at.localeCompare(a.entity.updated_at)).slice(0,6).map(item=><button className="history-card" key={item.entity.id} disabled={blockedNavigation} onClick={()=>void resume(item)}><span>RESEARCH · {item.entity.status}</span><h2>{item.entity.title}</h2><p>{String(item.details.objective||item.details.question||"")}</p><small>{new Date(item.entity.updated_at).toLocaleString()} · Continue →</small></button>)}</div>:<p>No research sessions yet. Start one to collect and connect evidence.</p>}</>}{capabilities.development&&<p className="home-development">Development sources and project memory are available from Development and Resume & memory.</p>}</section>}
      {clients.length > 0 && page === "board" && !compact && !preferredClient && <div className="ai-suggestion"><span>Available on this computer:</span>{clients.map(client => <button key={client.family} onClick={() => { if(client.family==="hermes")setAgent("hermes");else setPreferredClient(client.family); setPage("ai"); }}>Set up {client.name}</button>)}<small>Detected · not connected yet</small></div>}
      <div className="workspace-page" hidden={page !== "board" || compact}>
        {loading ? <p>Restoring your work…</p> : capabilities.research && (newResearch || (!sessions.length && !sessionId)) ? <form className="research-onboarding" onSubmit={start}>
          <p className="eyebrow">A NEW LINE OF INQUIRY</p><h1>What are you researching?</h1><p>Set the direction once. Then collect screenshots, notes, and evidence on your board.</p>
          <label>Short title<input autoFocus value={input.title} onChange={e => updateDraft("title", e.target.value)} maxLength={500} placeholder="e.g. Compare agent workflows" required /></label>
          <label>Question<textarea value={input.question} onChange={e => updateDraft("question", e.target.value)} maxLength={10000} placeholder="What do you want to learn?" required /></label>
          <details><summary>Context & desired outcome (optional)</summary><label>Context<textarea value={input.context} onChange={e => updateDraft("context", e.target.value)} maxLength={20000} /></label><label>Desired outcome<input value={input.desiredOutcome} onChange={e => updateDraft("desiredOutcome", e.target.value)} maxLength={20000} /></label></details>
          <button className="primary-action" disabled={saving || !input.title.trim() || !input.question.trim()}>{saving ? "Starting…" : "Start research →"}</button><small>Draft saved on this device as you type.</small>
        </form> : <>
          {capabilities.research && !sessionId && <div className="resume-banner"><h2>Continue a previous research</h2><p>Your saved sources are below. Choose a research from History or start a new one.</p><button onClick={() => setPage("history")}>Open research history</button></div>}
          <div className="board-layout" onDragOver={event => event.preventDefault()} onDrop={event => { event.preventDefault(); if (!sessionId) { setError("Select a research session before dropping screenshots."); return; } Array.from(event.dataTransfer.files).slice(0,8).forEach(file => enqueueImage(file, file.name)); }}>
            {capabilities.research ? <ResearchBoard key={sessionId||scope} projectId={project.project_id} boardKey={sessionId||scope} aiDraft={aiDraft} items={activeItems} session={session} graph={graph?.type === "graph" ? graph.specification : undefined} onInspect={inspect} onAnalyze={() => void requestOrganization()} analyzing={organizing} revision={revision} /> : <ResearchBoard projectId={project.project_id} boardKey="development" items={developmentItems} session={null} graph={graph?.type==="graph"?graph.specification:undefined} onInspect={inspect} onAnalyze={()=>void requestOrganization()} analyzing={organizing} revision={revision} aiDraft={aiDraft}/>}
            {capabilities.research && tray && <aside className="capture-tray"><div><h3>Recent captures</h3><input aria-label="Search research sources" placeholder="Search sources or notes" value={search} onChange={e => setSearch(e.target.value)} /></div>{!photos.length && <p>Paste or drop screenshots onto the board. Your originals are saved immediately.</p>}{visible.filter(i => i.entity.entity_type === "evidence").slice(0,24).map(item => <article key={item.entity.id}><div className="capture-preview"><EvidenceImage item={item} projectId={project.project_id} /><button className="capture-details" onClick={() => inspect(item)}><strong>{item.entity.title}</strong><small>{String(item.details.annotation || "Add a note")}</small></button></div><button className="remove-capture" aria-label={`Remove ${item.entity.title}`} title="Remove from board; retain in history" onClick={() => void remove(item)}>×</button></article>)}{photos.length > 24 && <p>Showing 24 previews. Search to narrow the list.</p>}{nextPage !== null && <button onClick={() => void researchItems(sessionId || null, false, nextPage).then(result => { setItems(current => [...current, ...result.items]); setNextPage(result.next_offset); }).catch(cause => setError(message(cause)))}>Load more sources</button>}</aside>}
          </div>
        </>}
      </div>
      {capabilities.research && sessionId && (page === "board" || compact || recording || live) && <div className="research-dock"><div><span className={live ? "live-status" : ""}>● {live ? "Saving new clipboard images" : "Research session"}</span><small>{compact ? session?.entity.title : "Use your system screenshot shortcut, then copy the image."}</small></div><div className="dock-buttons"><button disabled={liveStarting} onClick={() => void toggleLive()}>{liveStarting ? "Starting…" : live ? "Stop live" : "Live screenshots"}</button><button onClick={() => void pasteClipboard()}>Paste image</button><button onClick={() => { setPage("capture"); if (compact) void toggleDock(); }}>Screen / audio</button><button onClick={() => void toggleDock()}>{compact ? "Open workspace" : "Float toolbar"}</button></div><label><input type="checkbox" checked={noteEach} onChange={e => setNoteEach(e.target.checked)} /> Ask for title & description after every capture</label>{!compact && <button disabled={!photos.length || organizing} onClick={() => void requestOrganization()}>{organizing ? "AI is analyzing…" : "Analyze & connect with AI"}</button>}{compact && <small>{photos.length} captures saved · {saveStatus}</small>}</div>}
      <div className="workspace-page" hidden={page !== "history" || compact}><h1>Research history</h1><p>Sources are saved as you work. Select a session to continue where you left off.</p><input aria-label="Search research history" placeholder="Find a research…" value={search} onChange={e => setSearch(e.target.value)} /><div className="history-grid">{sessions.filter(s => s.entity.title.toLowerCase().includes(search.toLowerCase())).map(item => <button className="history-card" key={item.entity.id} disabled={blockedNavigation} onClick={() => void resume(item)}><span>RESEARCH · {item.entity.status}</span><h2>{item.entity.title}</h2><p>{String(item.details.objective)}</p><small>{new Date(item.entity.updated_at).toLocaleString()} · Continue →</small></button>)}</div>{nextHistory !== null && <button onClick={() => void researchItems(null, true, nextHistory).then(result => { setSessions(current => [...current, ...result.items]); setNextHistory(result.next_offset); }).catch(cause => setError(message(cause)))}>Load older research</button>}</div>
      <div className="workspace-page" hidden={page !== "development" || compact}>{capabilities.development && <WorkspaceQuickStart research={false} development busy={busy} onQuestion={async () => {}} onRepository={async path => { const result = await onRepository(path); markChanged(); return result; }} onError={setError} />}{capabilities.development&&capabilities.research&&<ResearchBoard projectId={project.project_id} boardKey="development" items={developmentItems} session={null} graph={graph?.type==="graph"?graph.specification:undefined} onInspect={inspect} onAnalyze={()=>void requestOrganization()} analyzing={organizing} revision={revision} aiDraft={aiDraft}/>}</div>
      <div className="workspace-page" hidden={page !== "capture" || compact}>{capabilities.research && <CapturePanel projectId={project.project_id} projectOpen researchEnabled activeResearchSessionId={sessionId} onRecordingChange={setRecording} onCaptureStatusChange={setCaptureStatus} />}</div>
      {page === "reports" && !compact && <div className="workspace-page"><ReportStudio key={sessionId||scope} projectId={project.project_id} scope={scope} sessionId={sessionId||null} document={report} agent={agent}/></div>}
      <div className="workspace-page" hidden={page!=="messages"||compact}><Messages key={sessionId||scope} projectId={project.project_id} scope={scope} sessionId={sessionId||null} agent={agent}/></div>
      <div className="workspace-page" hidden={page !== "context" || compact}><div className="page-actions"><h1>Context & memory</h1><button disabled={saving} onClick={() => void exportMemory()}>Export Context Pack .md</button></div><MemoryPanel key={sessionId||scope} projectId={project.project_id} scope={scope} sessionId={sessionId||null} onBookmark={saveBookmark}/><details><summary>Inspect checkpoints & customize Context Pack</summary><ContinuityPanel projectOpen capabilities={capabilities} /></details></div>
      <div className="workspace-page" hidden={page !== "ai" || compact}><div className="assistant-request"><h2>In-app assistant</h2><p>Board analysis, Reports and Messages use the selected local Codex or Hermes CLI when you ask them to. The assistant uses bounded project context and safe CLI defaults; your personal model settings and MCP tools may not be used. You need a working login or provider. AI requests can consume substantial tokens, especially with images.</p><p>External AI apps connect to Continuum through the MCP access below. Installing a client or creating access does not prove that a tool call has succeeded. Hermes is optional.</p></div><AiContinuityPanel projectOpen capabilities={capabilities} preferredClient={preferredClient} onUpdated={markChanged} projectPath={project.path} visible={page === "ai"} /><p><strong>Where AI acts:</strong> Board analysis proposes descriptions and relationships for review; Reports drafts Markdown; Messages answers questions from bounded project context. These in-app actions use the selected local agent. External MCP access lets another AI client read allowed context and submit proposals. Neither path silently turns an AI suggestion into a verified fact.</p></div>
      <div className="workspace-page" hidden={page !== "settings" || compact}>{settings}</div>
    </div>
      {inspected && <div className="evidence-dialog" role="dialog" aria-modal="true" aria-label="Research item details"><div><button className="dialog-close" aria-label="Close research item details" onClick={dismissNote}>×</button><h2>Edit {inspected.entity.entity_type.replaceAll("_", " ")}</h2>{inspected.entity.entity_type === "evidence" && <EvidenceImage item={inspected} projectId={project.project_id} thumbnail={false} />}<label>{inspected.entity.entity_type === "evidence" ? "What should this evidence be called?" : "Title"}<input autoFocus={inspected.entity.entity_type === "evidence"} readOnly={!["research_session","research_question","evidence"].includes(inspected.entity.entity_type)} value={editTitle} onChange={event => setEditTitle(event.target.value)} maxLength={500} placeholder={inspected.entity.entity_type === "evidence" ? "e.g. OpenAI realtime transcription pricing" : undefined} /></label><label>{inspected.entity.entity_type === "research_session" ? "Research objective" : inspected.entity.entity_type === "research_question" ? "Research question" : "Description / context for AI"}<textarea autoFocus={inspected.entity.entity_type !== "evidence"} readOnly={!["research_session","research_question","evidence"].includes(inspected.entity.entity_type)} value={note} onChange={e => setNote(e.target.value)} maxLength={10000} placeholder={inspected.entity.entity_type === "evidence" ? "What does this show, and why did you capture it? AI will compare this note and title with the image." : "Describe the research direction clearly."} /></label><button hidden={!["research_session","research_question","evidence"].includes(inspected.entity.entity_type)} disabled={saving || !editTitle.trim() || (inspected.entity.entity_type !== "evidence" && !note.trim())} onClick={() => void saveNote()}>{inspected.entity.entity_type === "evidence" ? "Save evidence" : "Save changes"}</button><button onClick={dismissNote}>{inspected.entity.entity_type === "evidence" ? "Skip for now" : "Cancel"}</button><small>The title is your intent label. AI checks whether the image supports it and proposes relationships; nothing becomes verified until you accept it.</small></div></div>}
      {exitState!=="idle"&&<div className="exit-overlay" role="dialog" aria-modal="true" aria-labelledby="exit-title"><div><h2 id="exit-title">{exitState==="saving"?"Saving before closing…":"Continuum could not close yet"}</h2><p>{exitState==="saving"?"Please wait while your latest workspace edits and bookmark are saved.":"Your project is still open. Resolve the issue below, or explicitly choose to close without the latest unsaved work."}</p>{exitError&&<p role="alert">{exitError}</p>}{exitState==="blocked"&&<div className="exit-actions"><button onClick={()=>void attemptExit.current()}>Try saving and close again</button><button onClick={()=>{setExitState("idle");setExitError("");}}>Keep working</button><button className="exit-danger" onClick={()=>void exitWithoutSaving()}>Close without saving latest work</button></div>}</div></div>}
  </div>;
}
