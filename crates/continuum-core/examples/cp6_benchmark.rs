use std::time::Instant;

use continuum_core::{
    ActorRef, CommandContext, ContinuityStore, GraphDirection, GraphQuery, NewEntity, OriginKind,
    new_id,
};
use serde_json::json;

fn percentile(samples: &mut [f64], percentile: f64) -> f64 {
    samples.sort_by(f64::total_cmp);
    let index = ((samples.len() - 1) as f64 * percentile).ceil() as usize;
    samples[index]
}

fn main() {
    let directory = tempfile::tempdir().expect("temporary benchmark directory");
    let store = ContinuityStore::create(
        directory.path().join("project"),
        "CP6 graph-500-v1 benchmark",
    )
    .expect("create benchmark project");
    let root = store
        .create_entity(&new_id(), NewEntity::authored("benchmark_node", "Root"))
        .expect("create root");
    let mut nodes = Vec::with_capacity(499);
    for index in 0..499 {
        nodes.push(
            store
                .create_entity(
                    &new_id(),
                    NewEntity::authored("benchmark_node", format!("Node {index}")),
                )
                .expect("create node"),
        );
    }
    for node in &nodes {
        store
            .create_relationship(&new_id(), "relates", &root, node, OriginKind::User)
            .expect("create root edge");
    }
    for index in 0..1_000 {
        let source = &nodes[index % nodes.len()];
        let target = &nodes[(index * 37 + 1) % nodes.len()];
        if source == target {
            continue;
        }
        store
            .create_relationship_with_context(
                &CommandContext::new(ActorRef::user("benchmark")),
                continuum_core::NewRelationship {
                    relation_type: format!("depends_on_{}", index / nodes.len()),
                    relation_version: 1,
                    source_entity_id: source.clone(),
                    target_entity_id: target.clone(),
                    origin: OriginKind::User,
                    confidence: Some(1.0),
                    review_state: continuum_core::RelationshipReviewState::Accepted,
                    direct_source_ids: vec![],
                    supersedes_id: None,
                },
            )
            .expect("create graph edge");
    }

    let one_hop = GraphQuery {
        root_entity_ids: vec![root.clone()],
        direction: GraphDirection::Both,
        max_depth: 1,
        max_nodes: 500,
        max_edges: 1_500,
        ..Default::default()
    };
    let scoped = GraphQuery {
        max_depth: 2,
        ..one_hop.clone()
    };
    store
        .traverse_provenance(one_hop.clone())
        .expect("warm one-hop query");
    store
        .traverse_provenance(scoped.clone())
        .expect("warm scoped query");

    let mut one_hop_ms = Vec::new();
    let mut scoped_ms = Vec::new();
    for _ in 0..30 {
        let started = Instant::now();
        let graph = store
            .traverse_provenance(one_hop.clone())
            .expect("one-hop query");
        assert_eq!(graph.nodes.len(), 500);
        one_hop_ms.push(started.elapsed().as_secs_f64() * 1_000.0);

        let started = Instant::now();
        let graph = store
            .traverse_provenance(scoped.clone())
            .expect("scoped query");
        assert_eq!(graph.nodes.len(), 500);
        assert!(graph.edges.len() >= 1_400);
        scoped_ms.push(started.elapsed().as_secs_f64() * 1_000.0);
    }
    let validation_started = Instant::now();
    let validation = store
        .validate_provenance_graph()
        .expect("validate benchmark graph");
    let validation_ms = validation_started.elapsed().as_secs_f64() * 1_000.0;
    assert!(validation.is_healthy());

    let result = json!({
        "fixture": "graph-500-v1",
        "nodes": 500,
        "relationships": store.debug_connection().unwrap().query_row(
            "SELECT count(*) FROM relationships", [], |row| row.get::<_, i64>(0)
        ).unwrap(),
        "iterations": 30,
        "one_hop_ms": {
            "p50": percentile(&mut one_hop_ms.clone(), 0.50),
            "p95": percentile(&mut one_hop_ms, 0.95),
            "target_p95": 300.0
        },
        "scoped_500_node_ms": {
            "p50": percentile(&mut scoped_ms.clone(), 0.50),
            "p95": percentile(&mut scoped_ms, 0.95),
            "query_only_target_p95": 2000.0
        },
        "validation_ms": validation_ms,
        "healthy": validation.is_healthy()
    });
    println!("{}", serde_json::to_string_pretty(&result).unwrap());
}
