# tox-tui

A modern, secure terminal client for the [Tox](https://tox.chat) peer-to-peer
messenger — written in Rust, built to replace dead C/C++ clients like uTox and
qTox.

```
┌──────────────┬────────────────────────────────────┐
│ tox-tui      │ you        online                 │
│              │ id:a1b2c3d4…                       │
│ ── friends ──│ ── Chat ────────────────────────── │
│ ▶ ● alice    │ [14:02] alice: hey                 │
│   ○ bob      │ [14:02] you: hi!                   │
│              │                                    │
│ ── transfers │ Select a friend, or press 'a' to   │
│   ↓ note.txt │ add one.                           │
├──────────────┴────────────────────────────────────┤
│ > type a message…  (/help for commands)           │
└───────────────────────────────────────────────────┘
 friends:1/2  requests:0  F1 help · Tab focus · …
```

## Features

- **Identity** — generates, persists, and reloads your Tox ID / keys
  (`tox.save` + `identity.json`) across restarts
- **Friends** — send/accept/decline friend requests, live online status,
  nicknames, status messages, typing indicators
- **Messaging** — real-time text chat with local JSONL history per friend
- **File transfers** — chunk-based P2P transfers with accept/reject prompts
  and on-disk progress (saved under `~/Downloads/tox-downloads/`)
- **TUI** — responsive ratatui + crossterm interface: sidebar, chat pane,
  input bar, modal dialogs, help overlay
- **Themes** — 8 built-ins (`default`, `light`, `nord`, `gruvbox`,
  `solarized-dark`, `solarized-light`, `dracula`, `monochrome`) plus custom
  palettes in `themes.toml` next to your save data
- **Tox ID copy** — one-key copy via OSC 52 (works over SSH) with a full-ID
  dialog; falls back to `wl-copy` / `xclip` / `xsel` / `pbcopy`
- **Navigation** — number keys `1–9`/`0` jump to friends, `n`/`p` cycle,
  mouse click/scroll in the sidebar and chat
- **Async core** — tokio event loop bridges c-toxcore callbacks to the UI
  without freezing input

## Architecture

| Crate | Role |
|-------|------|
| `crates/tox-sys` | Raw FFI bindings to [c-toxcore](https://github.com/TokTok/c-toxcore) (`tox.h` 0.2.x) |
| `crates/tox-client` | Safe async wrapper: identity, history, commands/events, file-transfer state machine |
| `crates/tox-tui` | Terminal UI (ratatui + crossterm) |

The pure-Rust [`tox`](https://crates.io/crates/tox) crate (last release 2020)
lacks a high-level client API (messaging, file transfer, callbacks), so this
project binds directly to the actively maintained c-toxcore reference
implementation.

```
UI thread (tokio + crossterm)          core thread (tox_iterate loop)
        │                                      │
        │── ToxCommand ──►  mpsc  ──►──────────┤
        │                                      │  tox_iterate(user_data)
        │◄── ToxEvent  ◄──  mpsc  ◄─── callbacks
        │                                      │
```

## Building

### Prerequisites

- Rust stable (1.80+)
- C toolchain + CMake (for c-toxcore)
- `libsodium` (dev package)
- Optional: `libclang` if you switch `tox-sys` to bindgen

### Build c-toxcore (local prefix, no root required)

```bash
git clone --depth 1 --branch v0.2.24 https://github.com/TokTok/c-toxcore.git vendor/c-toxcore
cd vendor/c-toxcore && git submodule update --init --recursive && cd ../..

cmake -S vendor/c-toxcore -B vendor/c-toxcore/build \
  -DCMAKE_BUILD_TYPE=Release \
  -DCMAKE_INSTALL_PREFIX=$PWD/local \
  -DBUILD_TOXAV=OFF -DUNITTEST=OFF -DAUTOTEST=OFF \
  -DBOOTSTRAP_DAEMON=OFF -DDHT_BOOTSTRAP=OFF

cmake --build vendor/c-toxcore/build -j$(nproc)
cmake --install vendor/c-toxcore/build
```

Point the build at a different install with `TOXCORE_PREFIX=/path/to/prefix`.

### Build the client

```bash
cargo build --release
./target/release/tox-tui
```

### Tests

```bash
cargo test                 # unit + offline integration tests
```

Integration tests spin up **two Tox instances on loopback**, cross-bootstrap
them over UDP (`127.0.0.1`), and verify friend requests, bidirectional chat,
identity persistence, and a 64 KiB file transfer — no public network needed.

## Keybindings

| Key | Action |
|-----|--------|
| `Tab` / `←→` | Cycle focus (sidebar ↔ chat ↔ input) |
| `↑↓` / `j k` | Move selection / scroll chat |
| `1–9` / `0` | Jump to friend #1–9 / #10 |
| `n` / `p` (also `]` / `[`) | Next / previous friend |
| `Enter` | Accept request · focus input · send message |
| `a` | Add friend |
| `c` | Show + copy your Tox ID (OSC 52) |
| `t` | Theme picker |
| `f` | Send file to current friend |
| `r` | Decline friend request |
| `i` | Set nickname |
| `s` | Set status message |
| `F1` / `?` | Help overlay |
| `Esc` | Close dialog / clear input / leave input |
| `Ctrl+Q` / `Ctrl+C` | Quit (saves identity) |

Mouse: click sidebar rows to select, scroll chat to page history, click chat
to focus it.

### Slash commands (in the input bar)

```
/add <ToxID> [greeting]   send a friend request
/file <path>              send a file to the current friend
/me <text>                action (/me-style) message
/name <nick>              set nickname
/status <text>            set status message
/id                       show your Tox ID (dialog; y re-copies)
/copy                     copy your Tox ID to the clipboard
/theme [name]             list themes, or apply one by name
/themes                   same as /theme (list)
/clear                    clear visible chat (history on disk kept)
/help                     keybinding help
/quit                     exit
```

## Configuration

| Env var | Meaning |
|---------|---------|
| `TOX_TUI_DATA_DIR` | State directory (default: `~/.local/share/tox-tui`) |
| `TOX_TUI_NAME` | Nickname applied on first run |
| `TOX_TUI_STATUS` | Status message applied on first run |
| `TOXCORE_PREFIX` | c-toxcore install prefix (build-time) |
| `RUST_LOG` | Log filter (stderr), e.g. `tox_client=debug` |

State layout:

```
$TOX_TUI_DATA_DIR/
├── tox.save          # full Tox savedata (keys, friends, …)
├── identity.json     # human-readable address / public key
├── themes.toml       # custom theme palettes (seeded with an example)
└── history/
    └── <friend-pk>.jsonl   # append-only chat history
```

## Project goals

1. **Secure by default** — all crypto lives in c-toxcore; the client never
   handles plaintext keys outside the save file.
2. **Low resource usage** — single iterate loop (~1 ms ticks), no busy-polling
   UI (50 ms draw tick), stripped release binary ≈ 2.3 MB.
3. **Non-blocking** — network events update state via channels; keyboard input
   never waits on the wire.
4. **Offline-testable** — full protocol flows verified without internet.

## License

GPL-3.0-or-later (same as c-toxcore).
