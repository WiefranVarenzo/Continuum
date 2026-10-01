//! Render an existing project's report for local visual quality checks.
use continuum_core::{ContinuityStore, HumanDocumentRequest, render_human_document};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 3 { return Err("Usage: report_preview PROJECT OUTPUT_DIRECTORY".into()); }
    let store = ContinuityStore::open(&args[1])?;
    let document = store.compose_human_document(HumanDocumentRequest::integrated())?;
    let rendered = render_human_document(&document)?;
    let output = std::path::Path::new(&args[2]);
    std::fs::create_dir_all(output)?;
    std::fs::write(output.join("report.html"), rendered.html)?;
    std::fs::write(output.join("report.md"), rendered.markdown)?;
    std::fs::write(output.join("document.json"), serde_json::to_vec_pretty(&document)?)?;
    println!("{}", output.join("report.html").display());
    Ok(())
}
