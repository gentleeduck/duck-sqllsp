# Install the duck-sqllsp binary

The VS Code extension is a thin client; the real work happens in the
`duck-sqllsp` server binary. Install it with:

```bash
cargo install --git https://github.com/gentleeduck/duck-sqllsp duck-sqllsp
```

The binary lands in `~/.cargo/bin/duck-sqllsp` (or `~/.local/bin/`
depending on your toolchain). The extension probes both paths
automatically.

If the binary is somewhere else, set **`duckSqllsp.serverPath`** in
your settings (User or Workspace) to the absolute path.

Check the **status bar** in the bottom right -- the database icon
shows `starting` -> `connected` / `offline mode` once the server
spawns. Hover for details, click to restart.
