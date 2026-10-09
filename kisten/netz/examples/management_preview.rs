//! Isolated desktop-boundary preview for manual UI acceptance.
//! cargo run -p orchester-netz --features static-files --example management_preview -- <assets> <test-root> <launch-url-file>
use orchester_anwendung::OrchesterPaths;
use orchester_netz::EmbeddedServer;
use std::{env, path::PathBuf};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = env::args_os().skip(1);
    let assets = PathBuf::from(args.next().ok_or("assets required")?);
    let root = PathBuf::from(args.next().ok_or("isolated test root required")?);
    let launch_file = PathBuf::from(args.next().ok_or("launch URL file required")?);
    if !root.is_absolute() || !launch_file.is_absolute() {
        return Err("absolute test paths required".into());
    }
    std::fs::create_dir_all(root.join("workspace"))?;
    let mut server = EmbeddedServer::start(
        OrchesterPaths::new(root.join("home"), root.join("workspace")),
        assets,
    )
    .await?;
    std::fs::write(launch_file, server.launch_url())?;
    println!("Isolated preview ready at {}", server.origin());
    tokio::signal::ctrl_c().await?;
    server.shutdown().await?;
    Ok(())
}
