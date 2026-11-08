# tox-client

Safe async wrapper around c-toxcore for messenger clients. Command channel in,
event channel out, backed by tokio.

```rust,no_run
use tox_client::{spawn, ToxCommand, ToxConfig};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut tox = spawn(ToxConfig::default()).await?;
    println!("Tox ID: {}", tox.identity().address);
    tox.send(ToxCommand::Shutdown)?;
    Ok(())
}
```

Requires c-toxcore installed at build time (`TOXCORE_PREFIX`).

Part of [tox-tui](https://github.com/hakuxyz/tox-tui).
