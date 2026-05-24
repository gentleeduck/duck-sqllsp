//! LSP server entry: spawn tokio, register the backend, pump stdio.
//!
//! Shutdown discipline:
//!   - `prctl(PR_SET_PDEATHSIG, SIGTERM)` on Linux so the kernel kills
//!     this process if nvim (our parent) dies without sending exit/shutdown.
//!   - Race the tower-lsp serve loop against SIGTERM / SIGINT / SIGHUP.
//!     The first signal that wins triggers a clean drop of the runtime.
//!   - `serve` returns on stdin EOF too, so a well-behaved client (LSP
//!     `exit` notification or just closing the pipe) also unblocks us.

use dsl_server::Backend;
use tower_lsp::{LspService, Server};

pub fn run() -> anyhow::Result<()> {
  // Linux: if our parent (nvim) dies abruptly we want a SIGTERM so we
  // don't linger as a leaked process attached to no tty.
  #[cfg(target_os = "linux")]
  unsafe {
    // PR_SET_PDEATHSIG = 1, signal = SIGTERM. Failure is non-fatal.
    let _ = libc::prctl(1, libc::SIGTERM, 0, 0, 0);
  }

  let rt = tokio::runtime::Builder::new_multi_thread().enable_all().build()?;
  rt.block_on(async move {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();
    let (service, socket) = LspService::new(Backend::new);
    let serve = Server::new(stdin, stdout, socket).serve(service);

    #[cfg(unix)]
    {
      use tokio::signal::unix::{SignalKind, signal};
      let mut sigterm = signal(SignalKind::terminate()).ok();
      let mut sigint = signal(SignalKind::interrupt()).ok();
      let mut sighup = signal(SignalKind::hangup()).ok();
      tokio::select! {
          _ = serve => {}
          _ = async {
              if let Some(s) = sigterm.as_mut() { s.recv().await; }
              else { std::future::pending::<()>().await; }
          } => { tracing::info!("SIGTERM received, shutting down"); }
          _ = async {
              if let Some(s) = sigint.as_mut() { s.recv().await; }
              else { std::future::pending::<()>().await; }
          } => { tracing::info!("SIGINT received, shutting down"); }
          _ = async {
              if let Some(s) = sighup.as_mut() { s.recv().await; }
              else { std::future::pending::<()>().await; }
          } => { tracing::info!("SIGHUP received, shutting down"); }
      }
    }
    #[cfg(not(unix))]
    {
      serve.await;
    }
  });
  Ok(())
}
