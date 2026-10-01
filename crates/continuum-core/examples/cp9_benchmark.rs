use std::fs;
use std::time::Instant;

use continuum_core::{
    ActorRef, ArtifactClassification, CaptureBackendDescriptor, CaptureBufferPolicy,
    CaptureCapability, CaptureEncodingSettings, CapturePermissionStatus, CapturePressureSample,
    CaptureSourceKind, CaptureSourceRequest, CommandContext, ContinuityStore, NewCaptureMarker,
    NewCaptureSegment, NewCaptureSession, Space, evaluate_capture_pressure,
};
use serde_json::json;

const SEGMENTS: usize = 30;
const SEGMENT_BYTES: usize = 512 * 1024;

fn command() -> CommandContext {
    CommandContext::new(ActorRef::user("cp9-benchmark"))
}

fn percentile(samples: &[f64], value: f64) -> f64 {
    samples[((samples.len() - 1) as f64 * value).ceil() as usize]
}

fn main() {
    let directory = tempfile::tempdir().expect("temporary benchmark directory");
    let store = ContinuityStore::create_with_actor(
        directory.path().join("project"),
        "CP9 capture benchmark",
        ActorRef::user("benchmark-owner"),
    )
    .expect("create benchmark project");
    store
        .set_space_capability_with_context(&command(), Space::Research, true)
        .expect("enable Research Space");

    let capability = CaptureCapability {
        source_kind: CaptureSourceKind::Screen,
        capability_id: "benchmark-screen".into(),
        available: true,
        reason: None,
    };
    let mut session = store
        .create_capture_session(
            &command(),
            NewCaptureSession {
                research_session_id: None,
                sources: vec![CaptureSourceRequest {
                    source_kind: CaptureSourceKind::Screen,
                    capability_id: capability.capability_id.clone(),
                }],
                backend: CaptureBackendDescriptor {
                    backend_id: "benchmark-segment-source".into(),
                    backend_version: "1".into(),
                    platform: std::env::consts::OS.into(),
                    capabilities: vec![capability],
                },
                encoding: CaptureEncodingSettings::default(),
                buffer_policy: CaptureBufferPolicy::default(),
                legal_consent_acknowledged: true,
            },
        )
        .expect("create capture session");
    session = store
        .record_capture_permission(
            &command(),
            &session.id,
            CaptureSourceKind::Screen,
            CapturePermissionStatus::Granted,
            Some("benchmark-explicit-grant"),
            session.state_version,
        )
        .expect("record permission");
    session = store
        .begin_capture(
            &command(),
            &session.id,
            "benchmark-visible-indicator",
            session.state_version,
        )
        .expect("begin capture");

    let before_hwm_kib = linux_status_kib("VmHWM");
    let mut samples_ms = Vec::with_capacity(SEGMENTS);
    let mut first_segment = None;
    for sequence in 0..SEGMENTS {
        let mut bytes = vec![(sequence % 251) as u8; SEGMENT_BYTES];
        bytes[0..8].copy_from_slice(&(sequence as u64).to_le_bytes());
        let started = Instant::now();
        let segment = store
            .ingest_capture_segment(
                &command(),
                &session.id,
                NewCaptureSegment {
                    sequence: sequence as u32,
                    media_type: "video/webm".into(),
                    source_kinds: vec![CaptureSourceKind::Screen],
                    start_offset_ms: sequence as u64 * 5_000,
                    end_offset_ms: (sequence as u64 + 1) * 5_000,
                    complete: true,
                    recovery_note: None,
                },
                &bytes,
                ArtifactClassification::Internal,
            )
            .expect("persist capture segment");
        samples_ms.push(started.elapsed().as_secs_f64() * 1_000.0);
        if first_segment.is_none() {
            first_segment = Some(segment);
        }
    }
    let marker = store
        .add_capture_marker(
            &command(),
            &session.id,
            NewCaptureMarker {
                segment_id: first_segment.as_ref().map(|segment| segment.id.clone()),
                offset_ms: 2_500,
                label: "Benchmark bookmark".into(),
                note: "Deterministic fixture".into(),
            },
        )
        .expect("persist marker");
    store
        .promote_capture_marker_to_evidence(
            &command(),
            &marker.id,
            "Benchmark Evidence",
            "Marker promoted without copying the recording",
            "Capture traceability fixture",
            None,
        )
        .expect("promote marker");
    session = store
        .stop_capture(&command(), &session.id, session.state_version)
        .expect("stop capture");
    assert_eq!(session.state.as_str(), "completed");
    assert!(
        store
            .verify_integrity()
            .expect("integrity scan")
            .is_healthy()
    );

    samples_ms.sort_by(f64::total_cmp);
    let after_hwm_kib = linux_status_kib("VmHWM");
    let policy = CaptureBufferPolicy::default();
    let pressure_action = evaluate_capture_pressure(
        &policy,
        &CapturePressureSample {
            buffered_bytes: policy.max_buffer_bytes,
            pending_segments: policy.max_pending_segments,
        },
    );
    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "checkpoint":"CP9",
            "build":env!("CARGO_PKG_VERSION"),
            "os":std::env::consts::OS,
            "arch":std::env::consts::ARCH,
            "cpu":linux_value("/proc/cpuinfo", "model name"),
            "memory":linux_value("/proc/meminfo", "MemTotal"),
            "fixture":{"name":"segmented-capture-30x512k-v1","segments":SEGMENTS,"bytes_per_segment":SEGMENT_BYTES,"total_bytes":SEGMENTS*SEGMENT_BYTES},
            "protocol":{"samples":SEGMENTS,"network_included":false,"content_dedup_disabled_by_unique_payloads":true},
            "segment_persist_ms":{"p50":percentile(&samples_ms,0.50),"p95":percentile(&samples_ms,0.95),"max":samples_ms.last().copied().unwrap_or_default()},
            "memory_high_water_kib":{"before":before_hwm_kib,"after":after_hwm_kib,"delta":after_hwm_kib.zip(before_hwm_kib).map(|(after,before)|after.saturating_sub(before))},
            "bounds":{"max_buffer_bytes":policy.max_buffer_bytes,"max_segment_bytes":policy.max_segment_bytes,"max_pending_segments":policy.max_pending_segments,"action_at_limit":pressure_action.as_str()},
            "local_checkpoint_target_ms":{"segment_persist_p95":500},
            "correctness":"30 unique segments, marker-to-Evidence traceability, completed lifecycle, and final integrity scan healthy"
        }))
        .unwrap()
    );
}

fn linux_value(path: &str, key: &str) -> Option<String> {
    fs::read_to_string(path).ok().and_then(|content| {
        content.lines().find_map(|line| {
            let (name, value) = line.split_once(':')?;
            (name.trim() == key).then(|| value.trim().to_owned())
        })
    })
}

fn linux_status_kib(key: &str) -> Option<u64> {
    linux_value("/proc/self/status", key)
        .and_then(|value| value.split_whitespace().next()?.parse().ok())
}
