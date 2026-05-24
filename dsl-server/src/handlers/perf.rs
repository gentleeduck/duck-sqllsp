//! Per-handler latency timing.
//!
//! [`Guard`] starts a timer on construction and logs the elapsed
//! microseconds when it goes out of scope. One line at the top of a
//! handler is enough; the early-return paths still drop the guard.
//!
//! Logs land on the `dsl_server::perf` tracing target so users can
//! filter them in / out (`RUST_LOG=dsl_server::perf=debug`). Default
//! `info` level keeps them visible without flooding the wire.

use std::time::Instant;
use tower_lsp::lsp_types::Url;

pub struct Guard {
  name: &'static str,
  uri: Option<Url>,
  start: Instant,
}

impl Guard {
  pub fn new(name: &'static str) -> Self {
    Self { name, uri: None, start: Instant::now() }
  }

  pub fn with_uri(name: &'static str, uri: &Url) -> Self {
    Self { name, uri: Some(uri.clone()), start: Instant::now() }
  }
}

impl Drop for Guard {
  fn drop(&mut self) {
    let us = self.start.elapsed().as_micros();
    // `target` filters; one log line per handler call.
    match &self.uri {
      Some(u) => tracing::info!(
          target: "dsl_server::perf",
          handler = self.name,
          uri = %u,
          us = %us,
          "handler done",
      ),
      None => tracing::info!(
          target: "dsl_server::perf",
          handler = self.name,
          us = %us,
          "handler done",
      ),
    }
  }
}
