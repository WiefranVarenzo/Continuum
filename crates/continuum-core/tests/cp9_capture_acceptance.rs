use continuum_core::{
    ActorRef, ArtifactClassification, CORE_SCHEMA_VERSION, CaptureBackendDescriptor,
    CaptureBackpressureAction, CaptureBufferPolicy, CaptureCapability, CaptureDerivationKind,
    CaptureEncodingSettings, CaptureExternalEvidence, CaptureExternalKind, CapturePermissionStatus,
    CapturePressureSample, CaptureSessionState, CaptureSourceKind, CaptureSourceRequest,
    CommandContext, ContinuityStore, CoreError, NewCaptureDerivation, NewCaptureMarker,
    NewCaptureSegment, NewCaptureSegmentEvidence, NewCaptureSession, PageRequest, Space,
    evaluate_capture_pressure,
};
use serde_json::json;

fn user() -> CommandContext {
    CommandContext::new(ActorRef::user("cp9-user"))
}

fn setup() -> (tempfile::TempDir, ContinuityStore) {
    let directory = tempfile::tempdir().unwrap();
    let store = ContinuityStore::create_with_actor(
        directory.path().join("project"),
        "CP9 capture fixture",
        ActorRef::user("owner"),
    )
    .unwrap();
    store
        .set_space_capability_with_context(&user(), Space::Research, true)
        .unwrap();
    (directory, store)
}

fn backend(sources: &[CaptureSourceKind]) -> CaptureBackendDescriptor {
    CaptureBackendDescriptor {
        backend_id: "webview-media-recorder".into(),
        backend_version: "1".into(),
        platform: "fixture".into(),
        capabilities: sources
            .iter()
            .map(|source_kind| CaptureCapability {
                source_kind: *source_kind,
                capability_id: format!("fixture-{}", source_kind.as_str()),
                available: true,
                reason: None,
            })
            .collect(),
    }
}

fn capture_request(sources: &[CaptureSourceKind], acknowledged: bool) -> NewCaptureSession {
    NewCaptureSession {
        research_session_id: None,
        sources: sources
            .iter()
            .map(|source_kind| CaptureSourceRequest {
                source_kind: *source_kind,
                capability_id: format!("fixture-{}", source_kind.as_str()),
            })
            .collect(),
        backend: backend(sources),
        encoding: CaptureEncodingSettings::default(),
        buffer_policy: CaptureBufferPolicy::default(),
        legal_consent_acknowledged: acknowledged,
    }
}

fn active_capture(
    store: &ContinuityStore,
    sources: &[CaptureSourceKind],
) -> continuum_core::CaptureSession {
    let mut session = store
        .create_capture_session(&user(), capture_request(sources, true))
        .unwrap();
    for source in sources {
        session = store
            .record_capture_permission(
                &user(),
                &session.id,
                *source,
                CapturePermissionStatus::Granted,
                Some("os-permission-observed"),
                session.state_version,
            )
            .unwrap();
    }
    store
        .begin_capture(
            &user(),
            &session.id,
            "persistent-capture-banner",
            session.state_version,
        )
        .unwrap()
}

fn segment(sequence: u32, start: u64, end: u64, complete: bool) -> NewCaptureSegment {
    NewCaptureSegment {
        sequence,
        media_type: "video/webm".into(),
        source_kinds: vec![CaptureSourceKind::Screen],
        start_offset_ms: start,
        end_offset_ms: end,
        complete,
        recovery_note: (!complete).then(|| "MediaRecorder interruption".into()),
    }
}

#[test]
fn complete_recording_assembly_preserves_order_sources_and_retry_identity() {
    let (_dir,store)=setup();
    let session=active_capture(&store,&[CaptureSourceKind::Screen]);
    assert!(store.promote_capture_recording_to_evidence(&user(),&session.id,"Demo","").is_err());
    let a=store.ingest_capture_segment(&user(),&session.id,segment(0,0,5000,true),b"header",ArtifactClassification::Internal).unwrap();
    store.ingest_capture_segment(&user(),&session.id,segment(1,5000,9000,true),b"continuation",ArtifactClassification::Internal).unwrap();
    let current=store.get_capture_session(&session.id).unwrap();
    store.stop_capture(&user(),&session.id,current.state_version).unwrap();
    let command=user();
    let evidence=store.promote_capture_recording_to_evidence(&command,&session.id,"Demo","Experiment result").unwrap();
    let artifact=evidence.details["original_artifact_id"].as_str().unwrap();
    assert_eq!(store.read_artifact_bounded(artifact,100).unwrap(),b"headercontinuation");
    assert_eq!(store.read_artifact_bounded(&a.artifact_id,100).unwrap(),b"header");
    let again=store.promote_capture_recording_to_evidence(&command,&session.id,"Demo","Experiment result").unwrap();
    assert_eq!(evidence.entity.id,again.entity.id);
    assert_eq!(evidence.entity.metadata["capture_segment_ids"].as_array().unwrap().len(),2);
}

#[test]
fn recording_assembly_does_not_downgrade_private_sources() {
    let (_dir,store)=setup();let session=active_capture(&store,&[CaptureSourceKind::Screen]);
    let part=store.ingest_capture_segment(&user(),&session.id,segment(0,0,5000,true),b"header",ArtifactClassification::Internal).unwrap();
    let current=store.get_capture_session(&session.id).unwrap();store.stop_capture(&user(),&session.id,current.state_version).unwrap();
    store.set_artifact_classification(&user(),&part.artifact_id,ArtifactClassification::NeverSend,"Sensitive recording").unwrap();
    assert!(store.promote_capture_recording_to_evidence(&user(),&session.id,"Demo","").unwrap_err().to_string().contains("privacy"));
}

fn external(kind: CaptureExternalKind, title: &str) -> CaptureExternalEvidence {
    let has_bytes = matches!(
        kind,
        CaptureExternalKind::File | CaptureExternalKind::Screenshot
    );
    CaptureExternalEvidence {
        kind,
        title: title.into(),
        source_uri: matches!(
            kind,
            CaptureExternalKind::Browser | CaptureExternalKind::Web
        )
        .then(|| "https://example.invalid/source".into()),
        source_title: Some(title.into()),
        captured_at: Some("2026-09-09T00:00:00Z".into()),
        media_type: has_bytes.then(|| "image/png".into()),
        bytes: has_bytes.then(|| b"fixture-original-bytes".to_vec()),
        source_content: matches!(
            kind,
            CaptureExternalKind::Browser | CaptureExternalKind::Web
        )
        .then(|| "Captured page excerpt, treated as untrusted Evidence.".into()),
        annotation: String::new(),
        summary: "Captured source".into(),
        relevance: "CP9 acceptance".into(),
        research_session_id: None,
        research_question_id: None,
        capture_session_id: None,
        classification: ArtifactClassification::Internal,
        metadata: json!({"fixture":true}),
    }
}

#[test]
fn sensor_capture_requires_permission_consent_and_persistent_indicator() {
    let (_directory, store) = setup();
    let session = store
        .create_capture_session(&user(), capture_request(&[CaptureSourceKind::Screen], true))
        .unwrap();
    assert_eq!(session.state, CaptureSessionState::AwaitingPermission);
    assert!(matches!(
        store.begin_capture(&user(), &session.id, "indicator", session.state_version),
        Err(CoreError::Conflict(_))
    ));
    let ready = store
        .record_capture_permission(
            &user(),
            &session.id,
            CaptureSourceKind::Screen,
            CapturePermissionStatus::Granted,
            Some("os-granted"),
            session.state_version,
        )
        .unwrap();
    assert_eq!(ready.state, CaptureSessionState::Ready);
    assert!(matches!(
        store.begin_capture(&user(), &ready.id, "", ready.state_version),
        Err(CoreError::Validation(_))
    ));
    let capturing = store
        .begin_capture(&user(), &ready.id, "capture-banner", ready.state_version)
        .unwrap();
    assert_eq!(capturing.state, CaptureSessionState::Capturing);
    assert_eq!(capturing.indicator_id.as_deref(), Some("capture-banner"));
    let paused = store
        .pause_capture(&user(), &capturing.id, capturing.state_version)
        .unwrap();
    assert_eq!(paused.state, CaptureSessionState::Paused);
    assert!(paused.indicator_id.is_some());
    let resumed = store
        .resume_capture(&user(), &paused.id, "capture-banner", paused.state_version)
        .unwrap();
    let completed = store
        .stop_capture(&user(), &resumed.id, resumed.state_version)
        .unwrap();
    assert_eq!(completed.state, CaptureSessionState::Completed);
    assert!(completed.indicator_id.is_none());

    let unacknowledged = store
        .create_capture_session(
            &user(),
            capture_request(&[CaptureSourceKind::Microphone], false),
        )
        .unwrap();
    let ready = store
        .record_capture_permission(
            &user(),
            &unacknowledged.id,
            CaptureSourceKind::Microphone,
            CapturePermissionStatus::Granted,
            None,
            unacknowledged.state_version,
        )
        .unwrap();
    assert!(matches!(
        store.begin_capture(&user(), &ready.id, "capture-banner", ready.state_version),
        Err(CoreError::Validation(_))
    ));

    let failed = store
        .fail_capture(
            &user(),
            &ready.id,
            "backend_start_failed",
            "The capture backend could not start.",
            false,
            ready.state_version,
        )
        .unwrap();
    assert_eq!(failed.state, CaptureSessionState::Failed);
    assert_eq!(failed.failure_code.as_deref(), Some("backend_start_failed"));
    assert_eq!(
        failed.failure_message.as_deref(),
        Some("The capture backend could not start.")
    );
    assert!(failed.indicator_id.is_none());
}

#[test]
fn segments_and_markers_become_evidence_without_duplicating_media() {
    let (_directory, store) = setup();
    let session = active_capture(&store, &[CaptureSourceKind::Screen]);
    let command = user();
    let captured = store
        .ingest_capture_segment(
            &command,
            &session.id,
            segment(0, 0, 5_000, true),
            b"bounded-webm-segment",
            ArtifactClassification::Internal,
        )
        .unwrap();
    let retry = store
        .ingest_capture_segment(
            &command,
            &session.id,
            segment(0, 0, 5_000, true),
            b"bounded-webm-segment",
            ArtifactClassification::Internal,
        )
        .unwrap();
    assert_eq!(captured.id, retry.id);
    assert!(matches!(
        store.ingest_capture_segment(
            &command,
            &session.id,
            segment(0, 0, 5_000, true),
            b"different-segment",
            ArtifactClassification::Internal,
        ),
        Err(CoreError::Conflict(_))
    ));

    let marker = store
        .add_capture_marker(
            &user(),
            &session.id,
            NewCaptureMarker {
                segment_id: Some(captured.id.clone()),
                offset_ms: 2_500,
                label: "Important observation".into(),
                note: "Return here during analysis.".into(),
            },
        )
        .unwrap();
    let segment_evidence = store
        .promote_capture_segment_to_evidence(
            &user(),
            NewCaptureSegmentEvidence {
                segment_id: captured.id.clone(),
                start_offset_ms: 2_000,
                end_offset_ms: 3_000,
                title: "One-second observation".into(),
                annotation: "Reviewed".into(),
                summary: "A bounded recording range".into(),
                relevance: "Supports the research question".into(),
                question_id: None,
            },
        )
        .unwrap();
    let marker_evidence = store
        .promote_capture_marker_to_evidence(
            &user(),
            &marker.id,
            "Research bookmark",
            "Timestamped observation",
            "Resume point",
            None,
        )
        .unwrap();
    assert_eq!(
        store
            .read_artifact_bounded(&captured.artifact_id, 1024)
            .unwrap(),
        b"bounded-webm-segment"
    );
    assert!(matches!(
        store.read_artifact_bounded(&captured.artifact_id, 4),
        Err(CoreError::Validation(_))
    ));
    assert_eq!(
        store
            .list_capture_markers(
                &session.id,
                PageRequest {
                    limit: 10,
                    offset: 0,
                },
            )
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        segment_evidence.details["original_artifact_id"].as_str(),
        Some(captured.artifact_id.as_str())
    );
    assert_eq!(marker_evidence.details["kind"], "capture_marker");
    let artifact_links: i64 = store
        .debug_connection()
        .unwrap()
        .query_row(
            "SELECT count(*) FROM entity_artifacts WHERE artifact_id=?1",
            [&captured.artifact_id],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(artifact_links, 1);
    assert!(store.verify_integrity().unwrap().is_healthy());
}

#[test]
fn interrupted_partial_segment_is_recoverable_after_reopen() {
    let (directory, store) = setup();
    let session = active_capture(&store, &[CaptureSourceKind::Screen]);
    let captured = store
        .ingest_capture_segment(
            &user(),
            &session.id,
            segment(0, 0, 1_700, false),
            b"partial-but-decodable-media",
            ArtifactClassification::Internal,
        )
        .unwrap();
    assert_eq!(captured.state, "recoverable");
    let current = store.get_capture_session(&session.id).unwrap();
    assert!(current.recoverable);
    let interrupted = store
        .interrupt_capture(
            &user(),
            &session.id,
            "application process ended",
            current.state_version,
        )
        .unwrap();
    assert_eq!(interrupted.state, CaptureSessionState::Interrupted);
    assert!(interrupted.indicator_id.is_none());
    drop(store);

    let reopened = ContinuityStore::open(directory.path().join("project")).unwrap();
    assert_eq!(
        reopened.get_capture_segment(&captured.id).unwrap().state,
        "recoverable"
    );
    assert!(
        reopened
            .get_capture_session(&session.id)
            .unwrap()
            .recoverable
    );
    assert!(reopened.verify_integrity().unwrap().is_healthy());
}

#[test]
fn file_web_browser_and_screenshot_paths_create_typed_evidence() {
    let (_directory, store) = setup();
    for (kind, expected) in [
        (CaptureExternalKind::File, "file"),
        (CaptureExternalKind::Screenshot, "screenshot"),
        (CaptureExternalKind::Web, "web"),
        (CaptureExternalKind::Browser, "web"),
    ] {
        let command = user();
        let input = external(kind, kind.as_str());
        let evidence = store
            .capture_external_evidence(&command, input.clone())
            .unwrap();
        assert_eq!(evidence.details["kind"], expected);
        assert_eq!(
            store
                .capture_external_evidence(&command, input)
                .unwrap()
                .entity
                .id,
            evidence.entity.id
        );
        let mut changed = external(kind, kind.as_str());
        changed.summary = "Changed retry".into();
        assert!(matches!(
            store.capture_external_evidence(&command, changed),
            Err(CoreError::Conflict(_))
        ));
    }
    assert!(store.verify_integrity().unwrap().is_healthy());
}

#[test]
fn permission_revocation_interrupts_capture_and_clears_indicator() {
    let (_directory, store) = setup();
    let session = active_capture(
        &store,
        &[CaptureSourceKind::Screen, CaptureSourceKind::Microphone],
    );
    assert!(matches!(
        store.record_capture_permission(
            &user(),
            &session.id,
            CaptureSourceKind::Microphone,
            CapturePermissionStatus::Granted,
            None,
            session.state_version,
        ),
        Err(CoreError::Conflict(_))
    ));
    let interrupted = store
        .record_capture_permission(
            &user(),
            &session.id,
            CaptureSourceKind::Microphone,
            CapturePermissionStatus::Revoked,
            Some("os-revoked"),
            session.state_version,
        )
        .unwrap();
    assert_eq!(interrupted.state, CaptureSessionState::Interrupted);
    assert!(interrupted.recoverable);
    assert!(interrupted.indicator_id.is_none());
}

#[test]
fn derived_ocr_or_transcript_never_replaces_the_original_capture() {
    let (_directory, store) = setup();
    let evidence = store
        .capture_external_evidence(
            &user(),
            external(CaptureExternalKind::Screenshot, "Source screenshot"),
        )
        .unwrap();
    let source_artifact = evidence.details["original_artifact_id"]
        .as_str()
        .unwrap()
        .to_string();
    let derivation = store
        .create_capture_derivation(
            &user(),
            NewCaptureDerivation {
                source_artifact_id: source_artifact.clone(),
                kind: CaptureDerivationKind::Ocr,
                engine_id: "fixture-ocr".into(),
                engine_version: "1".into(),
                settings: json!({"language":"en"}),
                media_type: "text/plain".into(),
                bytes: b"derived recognized text".to_vec(),
                classification: ArtifactClassification::Internal,
            },
        )
        .unwrap();
    assert_eq!(derivation.source_artifact_id, source_artifact);
    assert_ne!(
        derivation.source_artifact_id,
        derivation.derived_artifact_id
    );
    assert_eq!(
        store.get_artifact(&source_artifact).unwrap().availability,
        "available"
    );

    assert!(matches!(
        store.create_capture_derivation(
            &user(),
            NewCaptureDerivation {
                source_artifact_id: source_artifact,
                kind: CaptureDerivationKind::Thumbnail,
                engine_id: "fixture".into(),
                engine_version: "1".into(),
                settings: json!({}),
                media_type: "image/png".into(),
                bytes: b"fixture-original-bytes".to_vec(),
                classification: ArtifactClassification::Internal,
            },
        ),
        Err(CoreError::Validation(_))
    ));
}

#[test]
fn buffer_backpressure_bounds_pagination_migration_and_integrity_are_enforced() {
    let (_directory, store) = setup();
    let policy = CaptureBufferPolicy {
        max_buffer_bytes: 100,
        max_segment_bytes: 1_000,
        max_pending_segments: 2,
        quality_reduction_allowed: true,
    };
    assert_eq!(
        evaluate_capture_pressure(
            &policy,
            &CapturePressureSample {
                buffered_bytes: 20,
                pending_segments: 0,
            },
        ),
        CaptureBackpressureAction::Continue
    );
    assert_eq!(
        evaluate_capture_pressure(
            &policy,
            &CapturePressureSample {
                buffered_bytes: 80,
                pending_segments: 0,
            },
        ),
        CaptureBackpressureAction::FlushSegment
    );
    assert_eq!(
        evaluate_capture_pressure(
            &policy,
            &CapturePressureSample {
                buffered_bytes: 100,
                pending_segments: 2,
            },
        ),
        CaptureBackpressureAction::ReduceQuality
    );

    let mut request = capture_request(&[CaptureSourceKind::Screen], true);
    request.buffer_policy = CaptureBufferPolicy {
        max_buffer_bytes: 4,
        max_segment_bytes: 4,
        max_pending_segments: 1,
        quality_reduction_allowed: false,
    };
    let session = store.create_capture_session(&user(), request).unwrap();
    let ready = store
        .record_capture_permission(
            &user(),
            &session.id,
            CaptureSourceKind::Screen,
            CapturePermissionStatus::Granted,
            None,
            session.state_version,
        )
        .unwrap();
    let active = store
        .begin_capture(&user(), &ready.id, "indicator", ready.state_version)
        .unwrap();
    let mut duplicate_sources = segment(0, 0, 1_000, true);
    duplicate_sources.source_kinds = vec![CaptureSourceKind::Screen, CaptureSourceKind::Screen];
    assert!(matches!(
        store.ingest_capture_segment(
            &user(),
            &active.id,
            duplicate_sources,
            b"1234",
            ArtifactClassification::Internal,
        ),
        Err(CoreError::Validation(_))
    ));
    let mut wrong_container = segment(0, 0, 1_000, true);
    wrong_container.media_type = "video/mp4".into();
    assert!(matches!(
        store.ingest_capture_segment(
            &user(),
            &active.id,
            wrong_container,
            b"1234",
            ArtifactClassification::Internal,
        ),
        Err(CoreError::Validation(_))
    ));
    assert!(matches!(
        store.ingest_capture_segment(
            &user(),
            &active.id,
            segment(0, 0, 1_000, true),
            b"12345",
            ArtifactClassification::Internal,
        ),
        Err(CoreError::Validation(_))
    ));
    assert!(matches!(
        store.list_capture_sessions(PageRequest {
            limit: 101,
            offset: 0,
        }),
        Err(CoreError::Validation(_))
    ));
    let version: u32 = store
        .debug_connection()
        .unwrap()
        .query_row("SELECT max(version) FROM schema_migrations", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(version, CORE_SCHEMA_VERSION);
    assert!(store.verify_integrity().unwrap().is_healthy());
}

#[test]
fn version_nine_project_upgrades_atomically_to_capture_schema() {
    let (directory, store) = setup();
    drop(store);
    let ledger = directory.path().join("project/ledger.sqlite3");
    let connection = rusqlite::Connection::open(&ledger).unwrap();
    connection
        .execute_batch(
            "PRAGMA foreign_keys=OFF;
         DROP TABLE capture_derivations;
         DROP TABLE capture_external_items;
         DROP TABLE capture_evidence_links;
         DROP TABLE capture_markers;
         DROP TABLE capture_segments;
         DROP TABLE capture_permission_events;
         DROP TABLE capture_session_sources;
         DROP TABLE capture_sessions;
         DELETE FROM schema_migrations WHERE version=10;
         PRAGMA foreign_keys=ON;",
        )
        .unwrap();
    drop(connection);

    let upgraded = ContinuityStore::open(directory.path().join("project")).unwrap();
    let version: u32 = upgraded
        .debug_connection()
        .unwrap()
        .query_row("SELECT max(version) FROM schema_migrations", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(version, CORE_SCHEMA_VERSION);
    assert!(
        upgraded
            .create_capture_session(&user(), capture_request(&[CaptureSourceKind::Screen], true),)
            .is_ok()
    );
    assert!(upgraded.verify_integrity().unwrap().is_healthy());
}

#[test]
fn integrity_scan_detects_incomplete_capture_evidence_publication() {
    let (_directory, store) = setup();
    let session = active_capture(&store, &[CaptureSourceKind::Screen]);
    let captured = store
        .ingest_capture_segment(
            &user(),
            &session.id,
            segment(0, 0, 5_000, true),
            b"integrity-fixture-segment",
            ArtifactClassification::Internal,
        )
        .unwrap();
    let evidence = store
        .promote_capture_segment_to_evidence(
            &user(),
            NewCaptureSegmentEvidence {
                segment_id: captured.id,
                start_offset_ms: 1_000,
                end_offset_ms: 2_000,
                title: "Integrity fixture".into(),
                annotation: String::new(),
                summary: "Temporary fixture".into(),
                relevance: "Integrity diagnostics".into(),
                question_id: None,
            },
        )
        .unwrap();
    let connection = store.debug_connection().unwrap();
    connection
        .execute_batch("DROP TRIGGER capture_evidence_links_immutable_delete")
        .unwrap();
    connection
        .execute(
            "DELETE FROM capture_evidence_links WHERE evidence_entity_id=?1",
            [&evidence.entity.id],
        )
        .unwrap();
    let report = store.verify_integrity().unwrap();
    assert!(report.issues.iter().any(|issue| {
        issue.code == "capture_evidence_link_missing" && issue.path_or_id == evidence.entity.id
    }));
}
