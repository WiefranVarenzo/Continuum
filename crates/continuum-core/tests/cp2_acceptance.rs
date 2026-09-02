use std::fs;
use std::time::Duration;

use continuum_core::{
    ActorKind, ActorRef, ArtifactClassification, CheckpointScope, CommandContext, ContinuityStore,
    CoreError, EntityUpdate, NewEntity, NewRelationship, OriginKind, PageRequest,
    RelationshipPolicy, RelationshipReviewState, Space, new_id,
};
use rusqlite::params;
use serde_json::json;
use tempfile::TempDir;

fn project() -> (TempDir, ContinuityStore) {
    let temp = tempfile::tempdir().expect("temporary directory");
    let store = ContinuityStore::create(temp.path().join("project"), "Continuum Test")
        .expect("create project");
    (temp, store)
}

fn entity(store: &ContinuityStore, title: &str) -> String {
    store
        .create_entity(
            &new_id(),
            NewEntity {
                entity_type: "core.fixture".into(),
                schema_version: 1,
                title: title.into(),
                origin: OriginKind::User,
                metadata: json!({}),
                data: json!({"fixture": true}),
            },
        )
        .expect("create entity")
}

#[test]
fn project_bootstrap_is_local_isolated_and_migrated() {
    let (_temp, store) = project();
    assert!(store.root().join("continuum.project.json").is_file());
    assert!(store.root().join("ledger.sqlite3").is_file());
    assert!(store.root().join("artifacts/sha256").is_dir());
    assert!(!store.capability_enabled(Space::Research).unwrap());
    assert!(!store.capability_enabled(Space::Development).unwrap());
    assert_eq!(store.summary().unwrap().ledger_sequence, 1);

    let connection = store.debug_connection().unwrap();
    let version: i64 = connection
        .query_row("SELECT max(version) FROM schema_migrations", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(version, 2);
    let placeholder_count: i64 = connection
        .query_row("SELECT count(*) FROM entities", [], |row| row.get(0))
        .unwrap();
    assert_eq!(
        placeholder_count, 0,
        "spaces must not create synthetic entities"
    );

    let archive_command = new_id();
    store.set_project_archived(&archive_command, true).unwrap();
    store.set_project_archived(&archive_command, true).unwrap();
    assert_eq!(store.summary().unwrap().status, "archived");
    store.set_project_archived(&new_id(), false).unwrap();
    assert_eq!(store.summary().unwrap().status, "active");
}

#[test]
fn capability_command_is_idempotent_and_preserves_project_identity() {
    let (_temp, store) = project();
    let project_id = store.manifest().project_id.clone();
    let command = new_id();
    store
        .set_space_capability(&command, Space::Research, true)
        .unwrap();
    store
        .set_space_capability(&command, Space::Research, true)
        .unwrap();
    assert!(store.capability_enabled(Space::Research).unwrap());
    assert_eq!(store.summary().unwrap().project_id, project_id);

    let connection = store.debug_connection().unwrap();
    let events: i64 = connection
        .query_row(
            "SELECT count(*) FROM audit_events WHERE command_id=?1",
            [&command],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(events, 1);
}

#[test]
fn entity_writes_are_versioned_atomic_and_idempotent() {
    let (_temp, store) = project();
    let command = new_id();
    let input = NewEntity {
        entity_type: "core.note".into(),
        schema_version: 1,
        title: "First".into(),
        origin: OriginKind::Import,
        metadata: json!({"source": "fixture"}),
        data: json!({"text": "hello"}),
    };
    let first = store.create_entity(&command, input.clone()).unwrap();
    let retry = store.create_entity(&command, input).unwrap();
    assert_eq!(first, retry);

    let updated = store
        .update_entity(
            &new_id(),
            &first,
            EntityUpdate {
                title: "Second".into(),
                status: "active".into(),
                metadata: json!({"source": "fixture"}),
                data: json!({"text": "updated"}),
                expected_version: 1,
            },
        )
        .unwrap();
    assert_eq!(updated.version, 2);

    let before_sequence = store.summary().unwrap().ledger_sequence;
    let stale = store.update_entity(
        &new_id(),
        &first,
        EntityUpdate {
            title: "Must roll back".into(),
            status: "active".into(),
            metadata: json!({}),
            data: json!({}),
            expected_version: 1,
        },
    );
    assert!(matches!(stale, Err(CoreError::Conflict(_))));
    assert_eq!(store.get_entity(&first).unwrap().title, "Second");
    assert_eq!(store.summary().unwrap().ledger_sequence, before_sequence);
}

#[test]
fn relationships_require_endpoints_in_the_same_project() {
    let (_temp_a, store_a) = project();
    let (_temp_b, store_b) = project();
    let source = entity(&store_a, "Source");
    let local_target = entity(&store_a, "Local target");
    let foreign_target = entity(&store_b, "Foreign target");

    let command = new_id();
    let relationship = store_a
        .create_relationship(
            &command,
            "supports",
            &source,
            &local_target,
            OriginKind::User,
        )
        .unwrap();
    let retry = store_a
        .create_relationship(
            &command,
            "supports",
            &source,
            &local_target,
            OriginKind::User,
        )
        .unwrap();
    assert_eq!(relationship, retry);
    assert_eq!(
        store_a
            .get_relationship(&relationship)
            .unwrap()
            .relation_type,
        "supports"
    );

    let rejected = store_a.create_relationship(
        &new_id(),
        "supports",
        &source,
        &foreign_target,
        OriginKind::User,
    );
    assert!(matches!(rejected, Err(CoreError::Validation(_))));
}

#[test]
fn artifact_store_deduplicates_and_detects_corruption_with_guidance() {
    let (_temp, store) = project();
    let content = b"immutable evidence payload";
    let first = store
        .ingest_artifact(&new_id(), content, "text/plain")
        .unwrap();
    let second = store
        .ingest_artifact(&new_id(), content, "text/plain")
        .unwrap();
    assert_eq!(first.id, second.id);
    assert_eq!(first.sha256, second.sha256);
    let owner = entity(&store, "Artifact owner");
    let link_command = new_id();
    store
        .link_artifact_to_entity(&link_command, &owner, &first.id, "source")
        .unwrap();
    store
        .link_artifact_to_entity(&link_command, &owner, &first.id, "source")
        .unwrap();
    assert!(store.verify_integrity().unwrap().is_healthy());

    fs::write(store.root().join(&first.relative_path), b"tampered").unwrap();
    let report = store.verify_integrity().unwrap();
    assert!(!report.is_healthy());
    assert!(report.issues.iter().any(|issue| {
        issue.code == "artifact_hash_or_size_mismatch" && !issue.guidance.is_empty()
    }));
}

#[test]
fn checkpoints_are_scope_aware_immutable_bookmarks_not_end_states() {
    let (_temp, store) = project();
    store
        .set_space_capability(&new_id(), Space::Research, true)
        .unwrap();
    let source = entity(&store, "Active investigation");
    let before = store.summary().unwrap().ledger_sequence;
    let checkpoint = store
        .create_checkpoint(
            &new_id(),
            CheckpointScope::Research,
            &json!({
                "active_work": "Investigate storage",
                "unresolved": ["measure recovery time"],
                "next_actions": ["run fixture"]
            }),
            std::slice::from_ref(&source),
        )
        .unwrap();
    assert_eq!(checkpoint.scope, "research");
    assert_eq!(checkpoint.ledger_sequence, before);
    assert!(store.summary().unwrap().ledger_sequence > checkpoint.ledger_sequence);

    let connection = store.debug_connection().unwrap();
    let mutation = connection.execute(
        "UPDATE checkpoints SET summary_json='{}' WHERE id=?1",
        [&checkpoint.id],
    );
    assert!(
        mutation.is_err(),
        "checkpoint mutation must be rejected by SQLite"
    );
}

#[test]
fn durable_jobs_are_bounded_idempotent_and_follow_lifecycle() {
    let (_temp, store) = project();
    let first = store
        .enqueue_job("integrity.scan", "daily", &json!({"depth": "full"}), 3)
        .unwrap();
    let retry = store
        .enqueue_job("integrity.scan", "daily", &json!({"depth": "full"}), 3)
        .unwrap();
    assert_eq!(first.id, retry.id);
    let running = store
        .claim_next_job(Duration::from_secs(30))
        .unwrap()
        .expect("queued job");
    assert_eq!(running.state, "running");
    assert_eq!(running.attempts, 1);
    let running = store
        .update_job_progress(&running.id, 5, Some(10), Some("halfway"))
        .unwrap();
    assert_eq!(running.progress_current, 5);
    assert_eq!(running.progress_total, Some(10));
    let finished = store.finish_job(&running.id, true, None).unwrap();
    assert_eq!(finished.state, "succeeded");
    assert!(
        store
            .claim_next_job(Duration::from_secs(30))
            .unwrap()
            .is_none()
    );

    let cancellable = store
        .enqueue_job("integrity.scan", "cancel-me", &json!({}), 2)
        .unwrap();
    let cancelled = store.request_job_cancellation(&cancellable.id).unwrap();
    assert_eq!(cancelled.state, "cancelled");

    let retryable = store
        .enqueue_job("integrity.scan", "retry-me", &json!({}), 2)
        .unwrap();
    let retryable_id = retryable.id.clone();
    let retryable = store
        .claim_next_job(Duration::from_secs(30))
        .unwrap()
        .unwrap();
    assert_eq!(retryable.id, retryable_id);
    let failed = store
        .finish_job(&retryable.id, false, Some("fixture"))
        .unwrap();
    assert_eq!(failed.state, "failed");
    assert_eq!(store.retry_failed_job(&failed.id).unwrap().state, "queued");
}

#[test]
fn command_event_and_entity_envelopes_preserve_actor_and_correlation() {
    let (_temp, store) = project();
    let mut command = CommandContext::new(ActorRef::user("user:researcher-01"));
    command.causation_id = Some(new_id());
    let id = store
        .create_entity_with_context(
            &command,
            NewEntity {
                entity_type: "core.authorship_fixture".into(),
                schema_version: 3,
                title: "Attributed work".into(),
                origin: OriginKind::User,
                metadata: json!({"language": "id"}),
                data: json!({"text": "owned"}),
            },
        )
        .unwrap();
    let entity = store.get_entity(&id).unwrap();
    assert_eq!(entity.schema_version, 3);
    assert_eq!(entity.created_by, "user:researcher-01");
    assert_eq!(entity.origin, "user");

    let mut retry = CommandContext::new(ActorRef::user("user:researcher-01"));
    retry.idempotency_key = command.idempotency_key.clone();
    let retry_id = store
        .create_entity_with_context(
            &retry,
            NewEntity {
                entity_type: "core.authorship_fixture".into(),
                schema_version: 3,
                title: "Attributed work".into(),
                origin: OriginKind::User,
                metadata: json!({"language": "id"}),
                data: json!({"text": "owned"}),
            },
        )
        .unwrap();
    assert_eq!(retry_id, id);

    let connection = store.debug_connection().unwrap();
    let event_id: String = connection
        .query_row(
            "SELECT id FROM audit_events WHERE command_id=?1",
            [&command.command_id],
            |row| row.get(0),
        )
        .unwrap();
    let event = store.get_audit_event(&event_id).unwrap();
    assert_eq!(event.aggregate_id.as_deref(), Some(id.as_str()));
    assert_eq!(event.actor_kind, "user");
    assert_eq!(event.actor_id, "user:researcher-01");
    assert_eq!(event.correlation_id, command.correlation_id);
    assert_eq!(event.causation_id, command.causation_id);

    let command_row: (i64, String, String, i64) = connection
        .query_row(
            "SELECT command_type_version,actor_kind,correlation_id,payload_schema_version
             FROM commands WHERE id=?1",
            [&command.command_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .unwrap();
    assert_eq!(command_row, (1, "user".into(), command.correlation_id, 1));
}

#[test]
fn relationship_contract_records_types_review_sources_and_supersession() {
    let (_temp, store) = project();
    let source = entity(&store, "Source");
    let target = entity(&store, "Target");
    let command = CommandContext::new(ActorRef::user("user:lead"));
    let first = store
        .create_relationship_with_context(
            &command,
            NewRelationship {
                relation_type: "supports".into(),
                relation_version: 1,
                source_entity_id: source.clone(),
                target_entity_id: target.clone(),
                origin: OriginKind::User,
                confidence: Some(0.8),
                review_state: RelationshipReviewState::Accepted,
                direct_source_ids: vec![source.clone()],
                supersedes_id: None,
            },
        )
        .unwrap();
    let relationship = store.get_relationship(&first).unwrap();
    assert_eq!(relationship.source_entity_type, "core.fixture");
    assert_eq!(relationship.target_entity_type, "core.fixture");
    assert_eq!(relationship.actor_id, "user:lead");
    assert_eq!(relationship.direct_source_ids, vec![source.clone()]);

    let second = store
        .create_relationship_with_context(
            &CommandContext::new(ActorRef::user("user:lead")),
            NewRelationship {
                relation_type: "supports".into(),
                relation_version: 2,
                source_entity_id: source,
                target_entity_id: target,
                origin: OriginKind::User,
                confidence: Some(0.95),
                review_state: RelationshipReviewState::Accepted,
                direct_source_ids: vec![],
                supersedes_id: Some(first.clone()),
            },
        )
        .unwrap();
    assert_eq!(store.get_relationship(&first).unwrap().status, "superseded");
    assert_eq!(
        store.get_relationship(&second).unwrap().supersedes_id,
        Some(first)
    );
    let page = store
        .list_relationships_for_entity(
            &store.get_relationship(&second).unwrap().source_entity_id,
            PageRequest {
                limit: 10,
                offset: 0,
            },
        )
        .unwrap();
    assert_eq!(page.items.len(), 2);
}

#[test]
fn relationship_policy_port_can_reject_domain_invalid_pairs() {
    struct DenyAll;
    impl RelationshipPolicy for DenyAll {
        fn validate_pair(
            &self,
            _source_entity_type: &str,
            _relation_type: &str,
            _target_entity_type: &str,
        ) -> std::result::Result<(), String> {
            Err("fixture policy rejection".into())
        }
    }

    let (_temp, store) = project();
    let source = entity(&store, "Source");
    let target = entity(&store, "Target");
    let result = store.create_relationship_with_policy(
        &CommandContext::new(ActorRef::user("user:policy-test")),
        NewRelationship {
            relation_type: "supports".into(),
            relation_version: 1,
            source_entity_id: source,
            target_entity_id: target,
            origin: OriginKind::User,
            confidence: None,
            review_state: RelationshipReviewState::Accepted,
            direct_source_ids: vec![],
            supersedes_id: None,
        },
        &DenyAll,
    );
    assert!(matches!(result, Err(CoreError::Validation(_))));
}

#[test]
fn artifact_policy_lifecycle_and_purge_remain_auditable() {
    let (_temp, store) = project();
    let command = CommandContext::new(ActorRef::user("user:security"));
    let artifact = store
        .ingest_artifact_reader_with_context(
            &command,
            std::io::Cursor::new(b"classified payload"),
            "text/plain",
            ArtifactClassification::Secret,
            OriginKind::User,
            &json!({"purpose": "fixture"}),
        )
        .unwrap();
    assert_eq!(artifact.classification, "secret");
    assert_eq!(artifact.availability, "available");
    assert_eq!(artifact.created_by, "user:security");

    let duplicate = store
        .ingest_artifact_reader_with_context(
            &CommandContext::system(),
            std::io::Cursor::new(b"classified payload"),
            "text/plain",
            ArtifactClassification::Internal,
            OriginKind::Deterministic,
            &json!({}),
        )
        .unwrap();
    assert_eq!(duplicate.id, artifact.id);
    assert_eq!(duplicate.classification, "secret");

    let reclassified = store
        .set_artifact_classification(
            &CommandContext::new(ActorRef::user("user:security")),
            &artifact.id,
            ArtifactClassification::Confidential,
            "reviewed declassification fixture",
        )
        .unwrap();
    assert_eq!(reclassified.classification, "confidential");

    let purged = store
        .purge_artifact_payload(
            &CommandContext::new(ActorRef::user("user:security")),
            &artifact.id,
        )
        .unwrap();
    assert_eq!(purged.availability, "purged_payload");
    assert!(!store.root().join(&purged.relative_path).exists());
    assert!(store.verify_integrity().unwrap().is_healthy());
}

#[test]
fn outbox_claim_failure_retry_and_completion_are_durable() {
    let (_temp, store) = project();
    let first = store
        .claim_next_outbox(Duration::from_secs(30))
        .unwrap()
        .expect("project-created outbox message");
    assert_eq!(first.attempts, 1);
    store
        .fail_outbox(&first.id, "temporary adapter failure")
        .unwrap();
    let retried = store
        .claim_next_outbox(Duration::from_secs(30))
        .unwrap()
        .unwrap();
    assert_eq!(retried.id, first.id);
    assert_eq!(retried.attempts, 2);
    store.complete_outbox(&retried.id).unwrap();
}

#[test]
fn entity_queries_are_explicitly_bounded_and_paginated() {
    let (_temp, store) = project();
    for index in 0..3 {
        entity(&store, &format!("Entity {index}"));
    }
    let first = store
        .list_entities(PageRequest {
            limit: 2,
            offset: 0,
        })
        .unwrap();
    assert_eq!(first.items.len(), 2);
    assert_eq!(first.next_offset, Some(2));
    let second = store
        .list_entities(PageRequest {
            limit: 2,
            offset: first.next_offset.unwrap(),
        })
        .unwrap();
    assert_eq!(second.items.len(), 1);
    assert_eq!(second.next_offset, None);
    assert!(
        store
            .list_entities(PageRequest {
                limit: 101,
                offset: 0
            })
            .is_err()
    );
}

#[test]
fn export_and_import_preserve_ids_history_and_hashes() {
    let (temp, store) = project();
    let entity_id = entity(&store, "Persistent entity");
    let target_id = entity(&store, "Persistent target");
    let updated = store
        .update_entity(
            &new_id(),
            &entity_id,
            EntityUpdate {
                title: "Persistent entity v2".into(),
                status: "active".into(),
                metadata: json!({}),
                data: json!({"versioned": true}),
                expected_version: 1,
            },
        )
        .unwrap();
    let relationship_id = store
        .create_relationship(
            &new_id(),
            "supports",
            &entity_id,
            &target_id,
            OriginKind::User,
        )
        .unwrap();
    let artifact = store
        .ingest_artifact(&new_id(), b"portable payload", "application/octet-stream")
        .unwrap();
    store
        .link_artifact_to_entity(&new_id(), &entity_id, &artifact.id, "source")
        .unwrap();
    let export = temp.path().join("export");
    store.export_project(&export).unwrap();
    let imported = ContinuityStore::import_export(&export, temp.path().join("restored")).unwrap();

    assert_eq!(imported.manifest().project_id, store.manifest().project_id);
    assert_eq!(
        imported.get_entity(&entity_id).unwrap().version,
        updated.version
    );
    assert_eq!(
        imported
            .get_relationship(&relationship_id)
            .unwrap()
            .source_entity_id,
        entity_id
    );
    assert_eq!(
        imported.get_artifact(&artifact.id).unwrap().sha256,
        artifact.sha256
    );
    assert_eq!(
        imported.summary().unwrap().ledger_sequence,
        store.summary().unwrap().ledger_sequence
    );
    let link_count: i64 = imported
        .debug_connection()
        .unwrap()
        .query_row("SELECT count(*) FROM entity_artifacts", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(link_count, 1);
    assert!(imported.verify_integrity().unwrap().is_healthy());
}

#[test]
fn migration_checksum_tampering_is_detected_without_changing_project_state() {
    let (_temp, store) = project();
    let before = store.summary().unwrap();
    let connection = store.debug_connection().unwrap();
    connection
        .execute(
            "UPDATE schema_migrations SET checksum='invalid' WHERE version=1",
            [],
        )
        .unwrap();
    drop(connection);
    let reopened = ContinuityStore::open(store.root());
    assert!(matches!(
        reopened,
        Err(CoreError::MigrationChecksum { version: 1 })
    ));

    let raw = rusqlite::Connection::open(store.root().join("ledger.sqlite3")).unwrap();
    let after_id: String = raw
        .query_row("SELECT id FROM projects", [], |row| row.get(0))
        .unwrap();
    assert_eq!(after_id, before.project_id);
}

#[test]
fn failed_multi_row_command_leaves_no_command_event_or_relationship() {
    let (_temp, store) = project();
    let source = entity(&store, "Source");
    let command = new_id();
    let before = store.summary().unwrap().ledger_sequence;
    let result =
        store.create_relationship(&command, "supports", &source, &new_id(), OriginKind::User);
    assert!(result.is_err());
    let connection = store.debug_connection().unwrap();
    let counts: (i64, i64, i64) = connection
        .query_row(
            "SELECT
                (SELECT count(*) FROM commands WHERE id=?1),
                (SELECT count(*) FROM audit_events WHERE command_id=?1),
                (SELECT count(*) FROM relationships)",
            params![command],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .unwrap();
    assert_eq!(counts, (0, 0, 0));
    assert_eq!(store.summary().unwrap().ledger_sequence, before);
}

#[test]
fn ai_proposals_cannot_bypass_canonical_acceptance() {
    let (_temp, store) = project();
    let command = CommandContext::new(ActorRef {
        kind: ActorKind::AiProposal,
        id: "candidate:fixture".into(),
    });
    let result = store.create_entity_with_context(
        &command,
        NewEntity {
            entity_type: "core.forbidden".into(),
            schema_version: 1,
            title: "Must remain a candidate".into(),
            origin: OriginKind::AiProposal,
            metadata: json!({}),
            data: json!({}),
        },
    );
    assert!(matches!(result, Err(CoreError::Validation(_))));
    let count: i64 = store
        .debug_connection()
        .unwrap()
        .query_row("SELECT count(*) FROM entities", [], |row| row.get(0))
        .unwrap();
    assert_eq!(count, 0);
}
