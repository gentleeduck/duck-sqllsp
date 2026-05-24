//! External `sql-formatter` (npm CLI) shell-out.
//!
//! Locates the binary on `PATH` first, then falls back to common install
//! locations so the LSP works regardless of how the editor launches it
//! (mason, asdf, ~/.local/bin). Returns `None` when the binary is missing
//! or the child exits non-zero -- callers should pass the input through
//! unchanged in that case.

use crate::style::FormatterStyle;
use std::io::Write;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// Hard cap on how long we wait for `sql-formatter` to finish. Beyond
/// this we kill the child and return None so the LSP stays responsive
/// even if the formatter hangs on pathological input.
const FORMATTER_TIMEOUT: Duration = Duration::from_secs(5);

/// Maximum input size we send to the formatter. Larger docs short-circuit
/// to the unchanged buffer so editing a huge dump never blocks the LSP.
const MAX_INPUT_BYTES: usize = 2 * 1024 * 1024;

/// Run the external `sql-formatter` against `input`, piping stdin/stdout.
/// Returns `None` if the binary is unavailable, input exceeds the size
/// cap, the child exceeds the timeout, or it exits non-zero.
pub fn run_sql_formatter(input: &str, style: &FormatterStyle) -> Option<String> {
  if input.len() > MAX_INPUT_BYTES {
    return None;
  }
  let binary = locate_binary()?;
  let config = style.to_json();
  let mut child = Command::new(binary)
    .args(["-l", style.language.as_str(), "-c", config.as_str()])
    .stdin(Stdio::piped())
    .stdout(Stdio::piped())
    .stderr(Stdio::null())
    .spawn()
    .ok()?;
  if let Some(stdin) = child.stdin.as_mut() {
    stdin.write_all(input.as_bytes()).ok()?;
  }
  // Drop stdin so the child sees EOF and starts processing.
  drop(child.stdin.take());

  // Poll for completion with a hard timeout.
  let started = Instant::now();
  loop {
    match child.try_wait() {
      Ok(Some(status)) => {
        if !status.success() {
          return None;
        }
        break;
      },
      Ok(None) => {
        if started.elapsed() >= FORMATTER_TIMEOUT {
          let _ = child.kill();
          let _ = child.wait();
          return None;
        }
        std::thread::sleep(Duration::from_millis(20));
      },
      Err(_) => return None,
    }
  }

  let out = child.wait_with_output().ok()?;
  String::from_utf8(out.stdout).ok()
}

/// Find `sql-formatter` on PATH; fall back to common install locations
/// (mason, ~/.local/bin, asdf shims) so the LSP works regardless of how
/// the editor launches it.
pub fn locate_binary() -> Option<String> {
  if let Some(p) = which_on_path("sql-formatter") {
    return Some(p);
  }
  if let Ok(home) = std::env::var("HOME") {
    for rel in &[".local/share/nvim/mason/bin/sql-formatter", ".local/bin/sql-formatter", ".asdf/shims/sql-formatter"] {
      let p = format!("{home}/{rel}");
      if std::path::Path::new(&p).is_file() {
        return Some(p);
      }
    }
  }
  None
}

/// Scan PATH directories for an executable with the given name.
pub fn which_on_path(name: &str) -> Option<String> {
  let path = std::env::var_os("PATH")?;
  for dir in std::env::split_paths(&path) {
    let candidate = dir.join(name);
    if candidate.is_file() {
      return Some(candidate.to_string_lossy().into_owned());
    }
  }
  None
}
