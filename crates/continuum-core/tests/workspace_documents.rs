use continuum_core::{ActorRef,CommandContext,ContinuityStore,CoreError};
use serde_json::json;
fn command()->CommandContext {CommandContext::new(ActorRef::user("workspace-test"))}

#[test]
fn board_report_and_messages_survive_reopen_and_backup_without_overwrite(){
    let temp=tempfile::tempdir().unwrap();let root=temp.path().join("project");
    let store=ContinuityStore::create(&root,"Workspace fixture").unwrap();
    for (key,kind) in [("board:session","board"),("report:research:session","report"),("messages:session","messages")] {
        assert!(store.workspace_document(key).unwrap().is_none());
        let first=store.save_workspace_document(&command(),key,kind,0,&json!({"text":"first"})).unwrap();assert_eq!(first.revision,1);
        let second=store.save_workspace_document(&command(),key,kind,1,&json!({"text":"second"})).unwrap();assert_eq!(second.revision,2);
        assert!(matches!(store.save_workspace_document(&command(),key,kind,1,&json!({"text":"stale"})),Err(CoreError::Conflict(_))));
        assert_eq!(store.workspace_document_revision(key,1).unwrap()["text"],"first");
    }
    let export=temp.path().join("export");store.export_project(&export).unwrap();
    let restored=ContinuityStore::import_export(&export,temp.path().join("restored")).unwrap();
    assert_eq!(restored.workspace_document("board:session").unwrap().unwrap().payload["text"],"second");
    drop(store);let reopened=ContinuityStore::open(root).unwrap();
    assert_eq!(reopened.workspace_document("report:research:session").unwrap().unwrap().revision,2);
    assert!(reopened.verify_integrity().unwrap().is_healthy());
}
#[test]
fn invalid_keys_kinds_payloads_and_cross_project_reads_are_rejected(){
    let temp=tempfile::tempdir().unwrap();let a=ContinuityStore::create(temp.path().join("a"),"A").unwrap();let b=ContinuityStore::create(temp.path().join("b"),"B").unwrap();
    assert!(a.save_workspace_document(&command(),"../outside","board",0,&json!({})).is_err());
    assert!(a.save_workspace_document(&command(),"valid","unknown",0,&json!({})).is_err());
    assert!(a.save_workspace_document(&command(),"valid","board",0,&json!([])).is_err());
    assert!(a.save_workspace_document(&command(),"big","report",0,&json!({"text":"a".repeat(2*1024*1024)})).is_err());
    a.save_workspace_document(&command(),"ai:one","messages",0,&json!({"answer":"a"})).unwrap();
    assert!(b.workspace_document("ai:one").unwrap().is_none());
    assert_eq!(a.recent_workspace_documents("ai:").unwrap().len(),1);
    assert!(b.recent_workspace_documents("ai:").unwrap().is_empty());
}

#[test]
fn upgrades_v14_with_a_pre_migration_backup(){
    let temp=tempfile::tempdir().unwrap();let root=temp.path().join("upgrade");
    let store=ContinuityStore::create(&root,"Upgrade fixture").unwrap();
    let ledger=root.join(&store.manifest().ledger);
    drop(store);
    // Reconstruct the immediately preceding schema in this disposable fixture only.
    let connection=rusqlite::Connection::open(&ledger).unwrap();
    connection.execute_batch("DROP TABLE workspace_document_revisions; DROP TABLE workspace_documents; DELETE FROM schema_migrations WHERE version=15;").unwrap();
    drop(connection);
    let upgraded=ContinuityStore::open(&root).unwrap();
    upgraded.save_workspace_document(&command(),"board:new","board",0,&json!({"positions":{}})).unwrap();
    let backups=std::fs::read_dir(root.join("backups")).unwrap().map(|e|e.unwrap().file_name().to_string_lossy().into_owned()).collect::<Vec<_>>();
    assert!(backups.iter().any(|name|name.starts_with("pre-migration-v14-to-v15-")));
    assert!(upgraded.verify_integrity().unwrap().is_healthy());
}
