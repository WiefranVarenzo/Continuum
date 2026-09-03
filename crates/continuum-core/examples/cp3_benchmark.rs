use std::fs;
use std::time::{Duration, Instant};

use continuum_core::{
    ActorRef, CommandContext, ContinuityStore, EvidenceKind, NewEvidence, NewResearchQuestion,
    NewResearchSession, OriginKind, PageRequest, QuestionKind, ResearchCheckpointInput,
    ResearchEntityKind, ResearchSearchQuery, ResearchTimelineFilter, Space,
};
use serde_json::json;

const QUESTION_COUNT: usize = 999;
const EVIDENCE_COUNT: usize = 999;
const WARMUPS: usize = 5;
const SAMPLES: usize = 30;

fn main() {
    let temporary = tempfile::tempdir().expect("benchmark temporary directory");
    let project_root = temporary.path().join("standard-research-fixture");
    let store = ContinuityStore::create_with_actor(
        &project_root,
        "CP3 Standard Research Fixture",
        ActorRef::user("benchmark-owner"),
    )
    .expect("create benchmark project");
    store
        .set_space_capability_with_context(&command(), Space::Research, true)
        .expect("enable Research Space");
    let session = store
        .create_research_session(
            &command(),
            NewResearchSession {
                title: "Benchmark session".into(),
                objective: "Measure deterministic Research Core operations.".into(),
                started_at: None,
                metadata: json!({"fixture":"standard-research-v1"}),
            },
        )
        .expect("create fixture session");
    seed_research(&store, &session.entity.id);
    let checkpoint = store
        .create_research_checkpoint(
            &command(),
            ResearchCheckpointInput {
                note: "Benchmark baseline".into(),
                next_actions: vec!["Continue benchmark".into()],
            },
        )
        .expect("create baseline checkpoint");

    for _ in 0..WARMUPS {
        run_search(&store);
        let _ = store.resume_research().expect("warm resume");
        let _ = store.generate_research_report().expect("warm report");
        let _ = store
            .research_timeline(ResearchTimelineFilter::default(), PageRequest::default())
            .expect("warm timeline");
    }

    let search = sample(|| run_search(&store));
    let timeline = sample(|| {
        let _ = store
            .research_timeline(ResearchTimelineFilter::default(), PageRequest::default())
            .expect("sample timeline");
    });
    let resume = sample(|| {
        let state = store.resume_research().expect("sample resume");
        assert_eq!(
            state.checkpoint.as_ref().map(|value| value.id.as_str()),
            Some(checkpoint.id.as_str())
        );
    });
    let report = sample(|| {
        let generated = store.generate_research_report().expect("sample report");
        assert!(generated.markdown.contains("Research Questions"));
    });
    let checkpoint_create = sample(|| {
        let _ = store
            .create_research_checkpoint(
                &command(),
                ResearchCheckpointInput {
                    note: "Measured checkpoint".into(),
                    next_actions: vec!["Resume fixture work".into()],
                },
            )
            .expect("sample checkpoint creation");
    });
    assert!(
        store
            .verify_integrity()
            .expect("integrity scan")
            .is_healthy()
    );

    let output = json!({
        "benchmark_version": 1,
        "build": env!("CARGO_PKG_VERSION"),
        "os": std::env::consts::OS,
        "arch": std::env::consts::ARCH,
        "cpu": linux_value("/proc/cpuinfo", "model name"),
        "memory": linux_value("/proc/meminfo", "MemTotal"),
        "fixture": {
            "name": "standard-research-v1",
            "research_sessions": 1,
            "research_questions": QUESTION_COUNT,
            "evidence": EVIDENCE_COUNT,
            "question_evidence_links": EVIDENCE_COUNT,
            "checkpoint_sources": 1 + QUESTION_COUNT + EVIDENCE_COUNT,
            "note": "CP2 50k entity / 150k relationship benchmark remains the core-scale regression fixture"
        },
        "protocol": {"warmups":WARMUPS,"samples":SAMPLES,"network_included":false},
        "milliseconds": {
            "metadata_text_search": summarize(search),
            "timeline_first_page": summarize(timeline),
            "research_resume_state": summarize(resume),
            "deterministic_research_report": summarize(report),
            "research_checkpoint_create": summarize(checkpoint_create)
        },
        "targets_ms": {
            "metadata_text_search": 500,
            "research_resume_state": 1000,
            "deterministic_research_report": 3000,
            "research_checkpoint_create": 1000
        },
        "correctness": "all samples completed; final Continuity and Research integrity scan was healthy"
    });
    println!("{}", serde_json::to_string_pretty(&output).unwrap());
}

fn seed_research(store: &ContinuityStore, session_id: &str) {
    let mut questions = Vec::with_capacity(QUESTION_COUNT);
    for index in 0..QUESTION_COUNT {
        let question = store
            .create_research_question(
                &command(),
                NewResearchQuestion {
                    title: format!("Research question {index:04}"),
                    kind: QuestionKind::Question,
                    question: format!(
                        "How should deterministic research operation {index:04} behave?"
                    ),
                    context: "Local-first standard benchmark context.".into(),
                    desired_outcome: "A traceable answer.".into(),
                    priority: (index % 5) as u8,
                    due_at: None,
                    session_id: Some(session_id.into()),
                    metadata: json!({"fixture_index":index}),
                },
            )
            .expect("seed question");
        questions.push(question.entity.id);
    }
    for index in 0..EVIDENCE_COUNT {
        store
            .create_evidence(
                &command(),
                NewEvidence {
                    title: format!("Evidence needle-{index:04}"),
                    kind: EvidenceKind::Note,
                    origin: OriginKind::User,
                    source_uri: None,
                    source_title: Some(format!("Benchmark observation {index:04}")),
                    source_author: Some("benchmark".into()),
                    captured_at: None,
                    capture_method: "benchmark_note".into(),
                    stable_reference: Some(format!("fixture://evidence/{index:04}")),
                    source_content: Some(format!("Deterministic observed value {index:04}.")),
                    annotation: "Generated locally for performance measurement.".into(),
                    summary: "Bounded fixture Evidence.".into(),
                    relevance: "Exercises deterministic metadata search.".into(),
                    original_artifact_id: None,
                    question_id: Some(questions[index % questions.len()].clone()),
                    session_id: Some(session_id.into()),
                    metadata: json!({"fixture_index":index}),
                },
            )
            .expect("seed evidence");
    }
}

fn run_search(store: &ContinuityStore) {
    let result = store
        .search_research(ResearchSearchQuery {
            text: "needle-0998".into(),
            entity_type: Some(ResearchEntityKind::Evidence),
            status: Some("available".into()),
            session_id: None,
            page: PageRequest::default(),
        })
        .expect("sample search");
    assert_eq!(result.items.len(), 1);
}

fn command() -> CommandContext {
    CommandContext::new(ActorRef::user("benchmark"))
}

fn sample(mut operation: impl FnMut()) -> Vec<Duration> {
    (0..SAMPLES)
        .map(|_| {
            let started = Instant::now();
            operation();
            started.elapsed()
        })
        .collect()
}

fn summarize(mut samples: Vec<Duration>) -> serde_json::Value {
    samples.sort_unstable();
    let percentile = |fraction: f64| {
        let index = ((samples.len() - 1) as f64 * fraction).ceil() as usize;
        samples[index].as_secs_f64() * 1000.0
    };
    json!({
        "p50": percentile(0.50),
        "p95": percentile(0.95),
        "max": samples.last().unwrap().as_secs_f64() * 1000.0
    })
}

fn linux_value(path: &str, key: &str) -> Option<String> {
    fs::read_to_string(path).ok().and_then(|content| {
        content.lines().find_map(|line| {
            let (name, value) = line.split_once(':')?;
            (name.trim() == key).then(|| value.trim().to_owned())
        })
    })
}
