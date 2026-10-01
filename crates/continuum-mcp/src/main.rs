use std::env;
use std::io;
use std::path::PathBuf;

use continuum_core::ContinuityStore;
use continuum_mcp::{ContinuumMcpServer, run_stdio};

fn main() {
    if let Err(error) = run() {
        eprintln!("continuum-mcp failed: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let project_root = project_root()?;
    let bearer_token = env::var("CONTINUUM_MCP_GRANT_TOKEN")
        .map_err(|_| "CONTINUUM_MCP_GRANT_TOKEN is required")?;
    let store = ContinuityStore::open(project_root)?;
    let server = ContinuumMcpServer::new(store, bearer_token);
    run_stdio(server, io::stdin().lock(), io::stdout().lock())?;
    Ok(())
}

fn project_root() -> Result<PathBuf, Box<dyn std::error::Error>> {
    let mut arguments = env::args_os().skip(1);
    let mut project = env::var_os("CONTINUUM_PROJECT_ROOT").map(PathBuf::from);
    while let Some(argument) = arguments.next() {
        if argument == "--project" {
            project = arguments.next().map(PathBuf::from);
        } else {
            return Err(format!("unknown argument: {}", argument.to_string_lossy()).into());
        }
    }
    project.ok_or_else(|| "--project or CONTINUUM_PROJECT_ROOT is required".into())
}
