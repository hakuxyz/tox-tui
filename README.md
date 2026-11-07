# tox-tui

a modern tox client that lives in your terminal :)

![main screen](docs/screenshot-main.png)

tox is peer-to-peer messaging with end-to-end encryption, no phone number, no
server holding your chats. most of the old clients (utox, qtox) are pretty much
dead, so i built this one in rust with ratatui.

## what it does

- identity that sticks around between restarts (tox.save + identity.json)
- friend requests, online status, nicknames, status messages
- real-time chat with local history saved as jsonl per friend
- file transfers that show progress on disk
- themes: default, light, nord, gruvbox, solarized dark/light, dracula,
  monochrome, plus your own in themes.toml
- one key to copy your tox id (works over ssh via osc 52)
- number keys to jump straight to a friend, mouse support too

## a quick look

**help overlay** (press `F1` or `?`)

![help](docs/screenshot-help.png)

**theme picker** (press `t`)

![themes](docs/screenshot-themes.png)

**copy your tox id** (press `c`)

![tox id](docs/screenshot-tox-id.png)

**add a friend** (press `a`)

![add friend](docs/screenshot-add-friend.png)

**nord theme**

![nord](docs/screenshot-nord.png)

**dracula theme**

![dracula](docs/screenshot-dracula.png)

## how it's put together

| crate | what it does |
|-------|----------------|
| `crates/tox-sys` | raw ffi bindings to [c-toxcore](https://github.com/TokTok/c-toxcore) |
| `crates/tox-client` | safe async wrapper: identity, history, commands/events, file transfers |
| `crates/tox-tui` | the terminal ui (ratatui + crossterm) |

there's a pure-rust `tox` crate on crates.io but it last shipped in 2020 and
never grew a real client api, so this project talks to c-toxcore directly.
crypto all lives in c-toxcore; the rust side just moves bytes around.

```
ui thread (tokio + crossterm)          core thread (tox_iterate loop)
        │                                      │
        │── ToxCommand ──►  mpsc  ──►──────────┤
        │                                      │  tox_iterate(user_data)
        │◄── ToxEvent  ◄──  mpsc  ◄─── callbacks
        │                                      │
```

## building it

you'll need rust stable (1.80+), a c toolchain with cmake, and libsodium.

first build c-toxcore into a local prefix (no root needed):

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

then the client:

```bash
cargo build --release
./target/release/tox-tui
```

point the build at a different install with `TOXCORE_PREFIX=/path/to/prefix`.

at runtime you may need:

```bash
export LD_LIBRARY_PATH=$PWD/local/lib
```

## tests

```bash
cargo test
```

the integration tests spin up two tox instances, bootstrap them to each other
over udp on 127.0.0.1, and check friend requests, chat, identity persistence,
and a 64 kib file transfer. no internet required, which is nice.

## keybindings

| key | action |
|-----|--------|
| `Tab` / `←→` | cycle focus (sidebar, chat, input) |
| `↑↓` / `j k` | move selection / scroll chat |
| `1-9` / `0` | jump to friend #1-9 / #10 |
| `n` / `p` (or `]` / `[`) | next / previous friend |
| `Enter` | accept request, focus input, send message |
| `a` | add friend |
| `c` | show + copy your tox id |
| `t` | theme picker |
| `f` | send a file |
| `r` | decline a friend request |
| `i` | set nickname |
| `s` | set status message |
| `F1` / `?` | help |
| `Esc` | close dialog / clear input / leave input |
| `Ctrl+Q` / `Ctrl+C` | quit (saves identity) |

mouse works too: click sidebar rows to select, scroll the chat, click chat to
focus it.

### slash commands

```
/add <ToxID> [greeting]   send a friend request
/file <path>              send a file to the current friend
/me <text>                action message
/name <nick>              set nickname
/status <text>            set status message
/id                       show your tox id (dialog)
/copy                     copy your tox id
/theme [name]             list themes or apply one
/themes                   same as /theme
/clear                    clear visible chat (history on disk kept)
/help                     keybinding help
/quit                     exit
```

## configuration

| env var | meaning |
|---------|---------|
| `TOX_TUI_DATA_DIR` | state directory (default `~/.local/share/tox-tui`) |
| `TOX_TUI_NAME` | nickname on first run |
| `TOX_TUI_STATUS` | status message on first run |
| `TOXCORE_PREFIX` | c-toxcore install prefix (build-time) |
| `RUST_LOG` | log filter on stderr, e.g. `tox_client=debug` |

where stuff lives:

```
$TOX_TUI_DATA_DIR/
├── tox.save          # full tox savedata (keys, friends, ...)
├── identity.json     # human-readable address / public key
├── themes.toml       # custom theme palettes (seeded with an example)
└── history/
    └── <friend-pk>.jsonl
```

## goals

1. secure by default. all crypto stays in c-toxcore; we never touch plaintext
   keys outside the save file.
2. light on resources. one iterate loop around 1 ms, ui redraws every 50 ms,
   release binary is a couple mb.
3. nothing blocks. network events come in over channels, keyboard input never
   waits on the wire.
4. testable offline. full protocol flows run without internet.

## license

GPL-3.0-or-later (same as c-toxcore).
