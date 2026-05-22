# duck-sqllsp — context dump for Claude

Paste this into the system / project memory of any Claude session to bring it up to speed on duck-sqllsp.

---

## What it is

**duck-sqllsp** is a native Rust API gateway. Sits between clients and backend services. Routes, terminates TLS, authenticates, rate-limits, caches, breaks circuits, observes. Hyper + Tokio + rustls under the hood.

CLI binary name: `duck-sqllsp`. Top-level Rust crate: `dgw`. Repo: `github.com/gentleeduck/duck-sqllsp`.

## Why it exists

Existing OSS gateways:
- **Kong / Tyk** — heavy, JVM-adjacent, Postgres-backed, Lua plugins.
- **Caddy** — single binary, Go, plugins require recompile.
- **Envoy** — incredible perf, brutal config + ops.
- **Vercel / Cloudflare / Fastly** — great DX but SaaS lock-in.

Gap duck-sqllsp fills: **single-binary, Rust-fast, self-hostable, JS-extendable** gateway with Vercel-grade DX.

## Place in the gentleduck org

Sibling repos (all under `gentleeduck` GitHub org):
- `duck-ui` — React component library + headless primitives
- `duck-iam` — type-safe RBAC + ABAC + ReBAC authorization engine
- `duck-upload` — resumable file upload engine
- `duck-md` — Rust MDX compiler with velite-shaped TS API
- `duck-sqllsp` — **this project**
- `duck-ttlog` — lock-free Rust logger w/ crash snapshots (separate concern; **not** the tracing layer for duck-sqllsp)

Maintainer: `wildduck2` (Ahmed). All MIT.

## Tech stack

| Layer | Choice |
| --- | --- |
| Runtime | Tokio |
| HTTP | Hyper 1.x |
| TLS | rustls + ACME via `instant-acme` |
| Router | radix tree via `matchit` |
| Middleware | tower::Layer composition |
| Config | TOML/YAML/JSON via `figment` (TS-native later) |
| Auth | `jsonwebtoken` + custom JWKS cache |
| Rate limit | `governor` + sharded DashMap |
| Cache | `moka` |
| Circuit breaker | `failsafe` |
| Tracing | `tracing` + OpenTelemetry (use the ecosystem; do NOT use `ttlog` for the tracing role) |
| Metrics | Prometheus exposition |

## Workspace layout

20 crates at repo root (no `crates/` parent dir):

```
duck-sqllsp/
├── dgw-core/             Types, errors, traits. Foundation.
├── dgw-config/           Config schema + multi-format loader
├── dgw-router/           Radix HTTP router
├── dgw-upstream/         Outbound connection pool
├── dgw-proxy/            Reverse-proxy primitive
├── dgw-balance/          Load balancing strategies
├── dgw-health/           Active + passive health checks
├── dgw-middleware/       Middleware trait + chain
├── dgw-auth/             JWT / API key / mTLS auth
├── dgw-ratelimit/        Token bucket + sliding window
├── dgw-cache/            HTTP response cache
├── dgw-circuit/          Circuit breaker
├── dgw-transform/        Header / path / body rewrites
├── dgw-cors/             CORS preflight
├── dgw-tls/              rustls + ACME
├── dgw-observability/    OTel tracing + Prometheus + req-id
├── dgw-ws/               WebSocket proxy
├── dgw-sse/              SSE streaming proxy
├── dgw-cli/              `duck-sqllsp` binary
└── dgw/                  Top-level facade (re-exports + Server)
```

Each crate has `Cargo.toml` + `README.md`. **No `src/` directories yet** — user writes code stage-by-stage.

## Performance targets (non-negotiable for v1)

| Metric | Target |
| --- | --- |
| Added p50 latency | < 300 µs |
| Added p99 latency | < 2 ms |
| Throughput | 50k+ rps / core |
| Memory idle | < 50 MB |
| Memory @ 10k rps | < 300 MB |
| Cold start | < 100 ms |
| Config reload | zero downtime |
| Hot-path allocations per request | 0 |

## Five-phase build plan

All in `STAGES/` dir of the repo. Each stage = one md file with: Goal, Concepts-to-learn-first, Prerequisites, Tasks checklist, Learning resources (RFCs/crates/books), Acceptance criteria, Design considerations, Pitfalls.

| Phase | Goal | Stages | When |
| --- | --- | --- | --- |
| **1 - MVP** | Working single-node gateway in prod | 01-20 | First |
| **2 - Hardening** | gRPC, HTTP/3, compression, perf, fuzz, shadow, retry | 21-28 | After MVP runs prod ≥ 1 month |
| **3 - Plugins** | JS (QuickJS) + WASM (wasmtime) plugin runtimes, TS config | 29-33 | After Phase 2 stable |
| **4 - Platform** | Service discovery, cluster mode, multi-tenant, dashboard, secrets, deploy artefacts | 34-40 | Scaling to a team / fleet |
| **5 - Edge** | WAF, bot detection, geo, multi-region, billing, replay, canary, DDoS | 41-48 | Demand-driven; each one a project |

## Phase 1 stage order (build top-down)

01 dgw-core → 02 dgw-config → 03 dgw-router → 04 dgw-upstream → 05 dgw-proxy → 06 dgw-balance → 07 dgw-health → 08 dgw-middleware → 09 dgw-auth → 10 dgw-ratelimit → 11 dgw-cache → 12 dgw-circuit → 13 dgw-transform → 14 dgw-cors → 15 dgw-tls → 16 dgw-observability → 17 dgw-ws → 18 dgw-sse → 19 dgw (facade) → 20 dgw-cli

## Design rules (apply to every crate)

- `#![forbid(unsafe_code)]` everywhere unless benchmarked otherwise.
- `#![warn(missing_docs)]` on lib roots.
- Errors via `thiserror`; user-facing via `miette`.
- Async only. No blocking on the request path.
- No `unwrap()` / `expect()` in library code (tests fine).
- Streaming bodies. Never buffer unless transform demands it.
- Zero-alloc hot path. Pre-compile regex / routes at config-load.
- Per-route chains built at config-load, not per-request.
- Lock-free counters (`AtomicU64`, sharded `DashMap`).
- Re-export public types only through `dgw`; consumers depend on one crate.

## Quick start the user wants

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

## CLI shape

```
duck-sqllsp run    --config FILE [--watch]   # boot the server
duck-sqllsp check  --config FILE             # validate config
duck-sqllsp reload --admin URL               # POST to admin to hot-reload
```

Exit codes: `0` ok, `1` config error, `2` runtime error, `3` reload failure.

## What is explicitly NOT in MVP (Phase 1)

Do not bolt these on early:
- gRPC, HTTP/3 (Phase 2)
- Plugins JS / WASM, TypeScript config (Phase 3)
- Service discovery, cluster mode, multi-tenancy, dashboard (Phase 4)
- WAF, bot detection, multi-region, billing, DDoS (Phase 5)

## Helpful when talking to Claude about this project

- duck-sqllsp uses `tracing` + OpenTelemetry for distributed tracing. **NOT** ttlog. ttlog is a separate sibling project for hot-path lock-free logging + crash snapshots.
- "dgw" is the crate prefix. CLI binary is "duck-sqllsp".
- Don't add features outside the phase the user is currently working on.
- Don't propose new crates unless one of the existing 20 is the wrong home.
- When in doubt, point at the relevant `STAGES/PHASE-N/NN-name.md` file.
- All commits go on `main` (or `master`) directly during early Phase 1, PR workflow once Phase 1 is published.
- Conventional commits enforced (`feat`, `fix`, `chore`, etc.). Husky `commit-msg` hook runs `commitlint`.

## Files to read if a session needs more context

- `README.md` — workspace overview
- `STAGES/README.md` — phase map
- `STAGES/PREREQUISITES.md` — concepts the user needs to learn first
- `STAGES/PHASE-1-MVP/01-dgw-core.md` (and onward) — the actual to-do list
- Each `dgw-*/README.md` — per-crate doc in @duck-md style
