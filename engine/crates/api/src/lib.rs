//! The engine's HTTP surface.
//!
//! Phase 4 fills in the paid routes. Phase 0 ships only `/health`, so the binary, the router,
//! and the smoke test exist before any money-handling endpoint does.
//!
//! Route map (CLAUDE.md section 1, PHASES.md phase 4):
//!
//! | Route | Payment |
//! |---|---|
//! | `GET  /v1/markets` | free |
//! | `POST /v1/markets` | paid: subsidy |
//! | `POST /v1/markets/:id/fills` | paid: 402 advertises the ceiling |
//! | `GET  /v1/markets/:id/price` | paid: read fee |
//! | `GET  /v1/agents/:addr/markets/:id/receipt` | free |
//! | `GET  /health`, `GET /metrics` | free |

#![forbid(unsafe_code)]

use axum::{routing::get, Json, Router};
use serde::Serialize;

/// Liveness payload. Deliberately says nothing about internal state: readiness and
/// reconciliation health get their own endpoints in Phase 5.
#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct Health {
    /// Always `"ok"` when the process can serve.
    pub status: &'static str,
    /// Build version, from Cargo.
    pub version: &'static str,
}

/// Build the router. Taking no arguments is a Phase 0 convenience; Phase 4 passes state.
pub fn router() -> Router {
    Router::new().route("/health", get(health))
}

async fn health() -> Json<Health> {
    Json(Health {
        status: "ok",
        version: env!("CARGO_PKG_VERSION"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Phase 0 smoke test. Phase 4 replaces this with the adversarial API suite.
    #[tokio::test]
    async fn health_reports_ok() {
        let Json(body) = health().await;
        assert_eq!(body.status, "ok");
    }
}
