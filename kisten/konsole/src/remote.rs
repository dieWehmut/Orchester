//! LAN listener and local administration for the remote delegate service.
use std::{
    io::{self, BufRead, Write},
    net::SocketAddr,
    time::Duration,
};

use axum::http::{header, HeaderValue, Method};
use orchester_anwendung::OrchesterPaths;
use orchester_netz::{RemoteControl, ServerContext, ServerControl};
use serde_json::{json, Value};
use tokio_util::sync::CancellationToken;
use tower_http::{cors::CorsLayer, services::ServeDir};

use crate::args::RemoteArgs;

pub async fn serve(args: RemoteArgs, json_output: bool) -> io::Result<()> {
    let origins = args
        .allow_origin
        .iter()
        .map(|s| origin(s))
        .collect::<io::Result<Vec<_>>>()?;
    let assets = args
        .assets
        .map(|path| {
            let path = path.canonicalize()?;
            if !path.is_dir() || !path.join("index.html").is_file() {
                return Err(io::Error::other(
                    "assets must be a built mobile directory containing index.html",
                ));
            }
            Ok(path)
        })
        .transpose()?;
    let workspace = args.workspace.canonicalize()?;
    let paths = OrchesterPaths::discover()?.with_workspace(&workspace);
    let remote = RemoteControl::with_allowed_roots(
        ServerContext::new(Some(paths), ServerControl::new()),
        workspace.clone(),
        args.allow_root,
    )?;
    if args.no_pairing {
        remote.disable_pairing();
    }
    let mut router = remote.router();
    if let Some(directory) = assets {
        router = router.fallback_service(ServeDir::new(directory));
    }
    if !origins.is_empty() {
        router = router.layer(
            CorsLayer::new()
                .allow_origin(origins)
                .allow_methods([Method::GET, Method::POST, Method::DELETE])
                .allow_headers([header::AUTHORIZATION, header::CONTENT_TYPE]),
        );
    }
    let listener = tokio::net::TcpListener::bind(SocketAddr::new(args.bind, args.port)).await?;
    let address = format!("http://{}", listener.local_addr()?);
    emit(
        json_output,
        json!({
            "event":"listening", "address":address, "workspace":workspace,
            "pairing_code":if args.no_pairing { None } else { Some(remote.pairing_code()) },
            "pairing_expires_at":remote.pairing_expires_at(),
        }),
    )?;
    if !json_output {
        eprintln!("Phone: use this computer's reachable LAN IP and port {}. HTTP is for a trusted LAN/VPN.", listener.local_addr()?.port());
        eprintln!("Local commands: pair | close | devices | revoke <device_id> | quit");
    }

    // A dedicated OS thread avoids blocking Tokio's shutdown on its stdin pool.
    // EOF leaves a background service running; only quit or an OS signal stops it.
    let (send, mut commands) = tokio::sync::mpsc::channel(16);
    std::thread::spawn(move || {
        for line in io::stdin().lock().lines() {
            let Ok(line) = line else {
                break;
            };
            if send.blocking_send(line).is_err() {
                break;
            }
        }
    });
    let stop = CancellationToken::new();
    let stopped = stop.clone();
    let mut server = tokio::spawn(async move {
        axum::serve(listener, router)
            .with_graceful_shutdown(stopped.cancelled_owned())
            .await
    });
    let signal = shutdown_signal();
    tokio::pin!(signal);
    let mut input_open = true;
    loop {
        tokio::select! {
            result = &mut server => {
                remote.shutdown().await;
                return result.map_err(io::Error::other)?;
            },
            _ = &mut signal => break,
            command = commands.recv(), if input_open => match command {
                Some(line) if line.trim() == "quit" => break,
                Some(line) => {
                    let value = match local_command(&remote, line.trim()) {
                        Ok(value) => value,
                        Err(error) => json!({"event":"error","message":error.to_string()}),
                    };
                    emit(json_output, value)?;
                },
                None => input_open = false,
            }
        }
    }
    // Close the listener before draining/cancelling delegate processes.
    stop.cancel();
    remote.shutdown().await;
    match tokio::time::timeout(Duration::from_secs(5), &mut server).await {
        Ok(result) => result.map_err(io::Error::other)?,
        Err(_) => {
            server.abort();
            Ok(())
        }
    }
}

fn origin(value: &str) -> io::Result<HeaderValue> {
    let invalid = || {
        io::Error::other(
            "allow-origin requires an exact HTTP(S) origin, e.g. http://localhost:4175",
        )
    };
    let url = reqwest::Url::parse(value).map_err(|_| invalid())?;
    if !matches!(url.scheme(), "http" | "https")
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || url.path() != "/"
        || url.origin().ascii_serialization() != value
    {
        return Err(invalid());
    }
    HeaderValue::from_str(value).map_err(|_| invalid())
}

fn local_command(remote: &RemoteControl, command: &str) -> io::Result<Value> {
    Ok(match command {
        "pair" => {
            json!({"event":"pairing", "pairing_code":remote.rotate_pairing_code()?, "pairing_expires_at":remote.pairing_expires_at()})
        }
        "close" => {
            remote.disable_pairing();
            json!({"event":"pairing_closed"})
        }
        "devices" => json!({"event":"devices", "devices":remote.devices()}),
        c if c.starts_with("revoke ") => {
            json!({"event":"device_revoked", "revoked":remote.revoke_device(c[7..].trim())?})
        }
        _ => {
            json!({"event":"help", "commands":["pair", "close", "devices", "revoke <device_id>", "quit"]})
        }
    })
}

fn emit(json_output: bool, value: Value) -> io::Result<()> {
    let mut out = io::stdout().lock();
    if json_output {
        writeln!(out, "{value}")?;
    } else {
        writeln!(out, "{}", serde_json::to_string_pretty(&value)?)?;
    }
    out.flush()
}

async fn shutdown_signal() {
    #[cfg(unix)]
    let terminate = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut signal) => {
                signal.recv().await;
            }
            Err(_) => std::future::pending::<()>().await,
        }
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();
    tokio::select! {
        _ = tokio::signal::ctrl_c() => {},
        _ = terminate => {},
    }
}
