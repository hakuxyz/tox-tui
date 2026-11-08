# tox-sys

Raw FFI bindings to [c-toxcore](https://github.com/TokTok/c-toxcore) (tox.h 0.2.x) for Rust.

Only the subset needed by messenger clients is exposed. Requires a installed
c-toxcore library at build time (`TOXCORE_PREFIX` or `./local`).

```toml
[build-dependencies]
# nothing extra: build.rs of this crate finds libtoxcore
```

See the [tox-tui repository](https://github.com/hakuxyz/tox-tui) for the full client.
