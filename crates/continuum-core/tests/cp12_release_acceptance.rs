use continuum_core::{ActorRef, CommandContext, ContinuityStore, CoreError, Space};

fn command() -> CommandContext {
    CommandContext::new(ActorRef::user("cp12-release-test"))
}

#[test]
fn configured_project_backup_export_restore_and_reopen_are_healthy() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().join("active-project");
    let store =
        ContinuityStore::create_with_actor(&root, "CP12 release fixture", ActorRef::user("owner"))
            .unwrap();
    store
        .set_space_capability_with_context(&command(), Space::Research, true)
        .unwrap();
    store
        .set_space_capability_with_context(&command(), Space::Development, true)
        .unwrap();

    let backup = store
        .backup_database(directory.path().join("backup.sqlite3"))
        .unwrap();
    let backup_connection = rusqlite::Connection::open(backup).unwrap();
    let backup_status: String = backup_connection
        .query_row("PRAGMA integrity_check", [], |row| row.get(0))
        .unwrap();
    assert_eq!(backup_status, "ok");

    let export = directory.path().join("verified-export");
    store.export_project(&export).unwrap();
    let restored_root = directory.path().join("restored-project");
    let restored = ContinuityStore::import_export(&export, &restored_root).unwrap();
    assert_eq!(restored.manifest().project_id, store.manifest().project_id);
    assert!(restored.capability_enabled(Space::Research).unwrap());
    assert!(restored.capability_enabled(Space::Development).unwrap());
    assert!(restored.verify_integrity().unwrap().is_healthy());
    drop(restored);

    let reopened = ContinuityStore::open(&restored_root).unwrap();
    assert!(reopened.verify_integrity().unwrap().is_healthy());
}

#[test]
fn export_and_import_reject_destinations_nested_inside_their_source() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().join("active-project");
    let store = ContinuityStore::create(&root, "CP12 nested path fixture").unwrap();

    assert!(matches!(
        store.export_project(root.join("nested-export")),
        Err(CoreError::Validation(_))
    ));

    let export = directory.path().join("safe-export");
    store.export_project(&export).unwrap();
    assert!(matches!(
        ContinuityStore::import_export(&export, export.join("nested-restore")),
        Err(CoreError::Validation(_))
    ));
}
