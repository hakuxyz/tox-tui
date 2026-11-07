// clipboard: osc52 first, then common clipboard tools

use std::io::Write;
use std::process::{Command, Stdio};

// try osc52 on the tty, then wl-copy / xclip / xsel / pbcopy
pub fn copy(text: &str) -> String {
    if osc52(text) {
        return "copied via terminal clipboard (OSC 52)".into();
    }
    if wayland(text) || x11(text) || macos(text) {
        return "copied via system clipboard".into();
    }
    "clipboard unavailable — select the ID manually".into()
}

// write the osc52 escape to /dev/tty (or stdout as fallback)
fn osc52(text: &str) -> bool {
    use base64::Engine;
    let b64 = base64::engine::general_purpose::STANDARD.encode(text.as_bytes());
    let seq = format!("\x1b]52;c;{b64}\x07");

    // Prefer the controlling TTY so we don't fight ratatui's stdout buffer.
    let tty_ok = std::fs::OpenOptions::new()
        .write(true)
        .open("/dev/tty")
        .and_then(|mut f| f.write_all(seq.as_bytes()).and_then(|_| f.flush()))
        .is_ok();
    if tty_ok {
        return true;
    }
    std::io::stdout()
        .write_all(seq.as_bytes())
        .and_then(|_| std::io::stdout().flush())
        .is_ok()
}

fn wayland(text: &str) -> bool {
    pipe_to("wl-copy", &[], text)
}

fn x11(text: &str) -> bool {
    pipe_to("xclip", &["-selection", "clipboard"], text)
        || pipe_to("xsel", &["--clipboard"], text)
}

fn macos(text: &str) -> bool {
    pipe_to("pbcopy", &[], text)
}

fn pipe_to(bin: &str, args: &[&str], text: &str) -> bool {
    Command::new(bin)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .and_then(|mut child| {
            child
                .stdin
                .as_mut()
                .ok_or_else(|| std::io::Error::other("no stdin"))?
                .write_all(text.as_bytes())?;
            child.wait()
        })
        .map(|s| s.success())
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn copy_returns_message() {
        let msg = copy("TEST-TOX-ID");
        assert!(!msg.is_empty());
    }
}
