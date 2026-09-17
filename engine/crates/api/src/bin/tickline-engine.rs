//! The engine binary.
//!
//! Phase 0 serves `/health` and nothing else. Configuration, actors, the ledger pool, and the
//! settlement worker arrive in Phases 4 and 5.

#![forbid(unsafe_code)]

use std::net::SocketAddr;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .json()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();

    let addr: SocketAddr = std::env::var("BIND_ADDR")
        .unwrap_or_else(|_| "0.0.0.0:8080".to_owned())
        .parse()?;

    let listener = tokio::net::TcpListener::bind(addr).await?;
    // Log the bound address, not the requested one: with port 0 only the listener knows it.
    let bound = listener.local_addr()?;
    tracing::info!(addr = %bound, "tickline engine listening");

    axum::serve(listener, api::router())
        .with_graceful_shutdown(shutdown())
        .await?;
    Ok(())
}

/// Ctrl-C means stop taking new work, not drop what is in flight.
async fn shutdown() {
    let _ = tokio::signal::ctrl_c().await;
    tracing::info!("shutdown signal received");
}
