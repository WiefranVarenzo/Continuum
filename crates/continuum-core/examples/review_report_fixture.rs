//! Review presentation drafts ONLY in a disposable copy, for end-to-end render QA.
use continuum_core::{ActorRef, CommandContext, ContinuityStore, ExternalProposalKind, PageRequest, ProposalReviewDecision};
fn main() -> Result<(),Box<dyn std::error::Error>> {
    let root = std::path::PathBuf::from(std::env::args().nth(1).ok_or("fixture path required")?).canonicalize()?;
    if !root.starts_with("/tmp/continuum-review-qa") { return Err("Only /tmp/continuum-review-qa copies are allowed".into()); }
    let store = ContinuityStore::open(root)?;
    let mut seen = std::collections::HashSet::new();
    for proposal in store.list_external_proposals(Some("pending"),PageRequest{limit:100,offset:0})?.items {
        if matches!(proposal.kind,ExternalProposalKind::ResearchSynthesis|ExternalProposalKind::DiagramPlan) && seen.insert(format!("{:?}",proposal.kind)) {
            store.review_external_proposal(&CommandContext::new(ActorRef::user("visual-qa")),&proposal.id,proposal.version,ProposalReviewDecision::Accept,"Disposable QA copy; original drafts remain pending")?;
            println!("Reviewed {:?} in disposable copy",proposal.kind);
        }
    }
    if seen.len()!=2 {return Err("Both synthesis and diagram drafts are required".into());}
    Ok(())
}
