<p align="center">
  <img src="./public/logo-dark.svg" alt="duck-sqllsp" width="120"/>
</p>

<h1 align="center">duck-sqllsp</h1>

<p align="center">
  Native Rust API gateway. Hyper + Tokio + rustls. Built as a small set of focused crates so each piece is reusable on its own.
</p>

<p align="center">
  <a href="./LICENSE">MIT</a> -
  <a href="./CHANGELOG.md">Changelog</a> -
  <a href="./CONTRIBUTING.md">Contributing</a> -
  <a href="./crates/dgw">Crate docs</a> -
  <a href="./examples">Examples</a>
</p>

<p align="center">
  <a href="https://crates.io/crates/dgw"><img src="https://img.shields.io/crates/v/dgw.svg" alt="crates.io"/></a>
  <a href="https://docs.rs/dgw"><img src="https://docs.rs/dgw/badge.svg" alt="docs.rs"/></a>
  <a href="./LICENSE"><img src="https://img.shields.io/crates/l/dgw.svg" alt="MIT"/></a>
</p>

---

## Install

```sh
cargo install duck-sqllsp
```

Or as a library:

```sh
cargo add dgw
```

## Quick start

```toml
# duck-sqllsp.toml
listen = "0.0.0.0:8080"

[[routes]]
host = "api.example.com"
path = "/v1/*rest"
upstream = "users-svc"
middleware = ["jwt-auth", "ratelimit:100rps", "cache:60s"]

[upstreams.users-svc]
endpoints = ["http://10.0.0.10:9000", "http://10.0.0.11:9000"]
balance = "least-conn"
health = { path = "/healthz", interval = "5s" }
```

```sh
duck-sqllsp run --config duck-sqllsp.toml
```

## Workspace

| Crate | Role |
| --- | --- |
| [`dgw-core`](crates/dgw-core) | Core types, error model, shared traits |
| [`dgw-config`](crates/dgw-config) | Config schema + loader (TOML / YAML / JSON / env) |
| [`dgw-router`](crates/dgw-router) | Radix HTTP router - host + path + method matching |
| [`dgw-upstream`](crates/dgw-upstream) | Upstream pool, keep-alive, HTTP/2 multiplexing |
| [`dgw-proxy`](crates/dgw-proxy) | Reverse-proxy primitive - forward, stream, hop-by-hop strip |
| [`dgw-balance`](crates/dgw-balance) | Load balancing: round-robin, least-conn, weighted, EWMA |
| [`dgw-health`](crates/dgw-health) | Active + passive health checks |
| [`dgw-middleware`](crates/dgw-middleware) | Middleware trait + composable chain |
| [`dgw-auth`](crates/dgw-auth) | JWT / API key / mTLS verification middleware |
| [`dgw-ratelimit`](crates/dgw-ratelimit) | Token-bucket + sliding-window rate limit |
| [`dgw-cache`](crates/dgw-cache) | HTTP response cache, stale-while-revalidate |
| [`dgw-circuit`](crates/dgw-circuit) | Circuit breaker per-upstream |
| [`dgw-transform`](crates/dgw-transform) | Header / path / body rewrite middleware |
| [`dgw-cors`](crates/dgw-cors) | CORS preflight + headers |
| [`dgw-tls`](crates/dgw-tls) | rustls termination + ACME auto-cert |
| [`dgw-observability`](crates/dgw-observability) | OTel tracing, Prometheus metrics, request IDs |
| [`dgw-ws`](crates/dgw-ws) | WebSocket proxy passthrough |
| [`dgw-sse`](crates/dgw-sse) | Server-Sent Events streaming proxy |
| [`dgw-cli`](crates/dgw-cli) | `duck-sqllsp` binary (run, check, reload) |
| [`dgw`](crates/dgw) | Top-level facade - wires the crates into one server |

## Examples

| Path | What it shows |
| --- | --- |
| [`examples/minimal`](examples/minimal) | Static route table, single upstream, no TLS |
| [`examples/jwt-auth`](examples/jwt-auth) | JWT verification + per-key rate limit |
| [`examples/tls-acme`](examples/tls-acme) | Auto-TLS via Let's Encrypt |

## Build

```sh
cargo build --release
cargo test  --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

## Performance targets

| Metric | Target |
| --- | --- |
| Added p50 latency | < 300 us |
| Added p99 latency | < 2 ms |
| Throughput | 50k+ rps / core |
| Memory idle | < 50 MB |
| Memory @ 10k rps | < 300 MB |
| Cold start | < 100 ms |
| Config reload | zero downtime |

## Design

- **Hyper + Tokio + rustls** core. Zero-copy where possible (`bytes::Bytes`).
- **Radix tree router** via `matchit`. Pre-compiled at config load, not per request.
- **Connection pooling** to upstreams w/ HTTP/2 multiplexing.
- **Lock-free hot path** (atomic counters, sharded `DashMap`).
- **Streaming bodies** - never buffer full body unless transform requires.
- **`tower::Layer`** for middleware composition.

JS / WASM plugin runtimes are not in this workspace yet; they ship as separate crates once the core stabilises.

## Sibling repos

[`@gentleduck/ui`](https://github.com/gentleeduck/duck-ui) -
[`@gentleduck/iam`](https://github.com/gentleeduck/duck-iam) -
[`@gentleduck/upload`](https://github.com/gentleeduck/duck-upload) -
[`@gentleduck/md`](https://github.com/gentleeduck/duck-md)

## Contributing

PR checklist + style notes in [`CONTRIBUTING.md`](CONTRIBUTING.md).
Security: [`SECURITY.md`](SECURITY.md). Behaviour: [`CODE_OF_CONDUCT.md`](CODE_OF_CONDUCT.md).

## License

MIT. See [`LICENSE`](LICENSE).
