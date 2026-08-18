# Install the duck-sqllsp binary

The VS Code extension is a thin client; the real work happens in the
`duck-sqllsp` server binary. Install it with:

```bash
cargo install duck-sqllsp
```

The binary lands in `~/.cargo/bin/duck-sqllsp` (`%USERPROFILE%\.cargo\bin\duck-sqllsp.exe`
on Windows) -- the extension probes that location, a few other common
install paths, and `$PATH` automatically.

If the binary is somewhere else, set **`duckSqllsp.serverPath`** in
your settings (User or Workspace) to the absolute path.

Check the **status bar** in the bottom right -- the database icon
shows `starting` -> `connected` / `offline mode` once the server
spawns. Hover for details, click to restart.
