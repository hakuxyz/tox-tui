// ratatui rendering: panes + dialogs

use crate::app::{App, Dialog, Focus};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Wrap};
use ratatui::Frame;

pub fn draw(f: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // header
            Constraint::Min(3),    // body
            Constraint::Length(1), // input
            Constraint::Length(1), // status
        ])
        .split(f.area());

    draw_header(f, app, chunks[0]);
    draw_body(f, app, chunks[1]);
    draw_input(f, app, chunks[2]);
    draw_status(f, app, chunks[3]);

    match app.dialog {
        Dialog::None => {}
        Dialog::Help => draw_help(f, app),
        Dialog::AddFriend => draw_add_friend(f, app),
        Dialog::SendFile => draw_send_file(f, app),
        Dialog::IncomingFile => draw_incoming_file(f, app),
        Dialog::SetName => draw_prompt(f, app, "Nickname", &app.dialog_input),
        Dialog::SetStatus => draw_prompt(f, app, "Status message", &app.dialog_input),
        Dialog::FriendRequest => draw_friend_request(f, app),
        Dialog::ShowId => draw_show_id(f, app),
        Dialog::ThemePicker => draw_theme_picker(f, app),
    }
}

fn draw_header(f: &mut Frame, app: &App, area: Rect) {
    let th = &app.theme;
    let conn = app.self_connection;
    let (conn_style, conn_text) = match conn {
        c if c.is_online() => (Style::default().fg(th.online()), c.label().to_string()),
        _ => (Style::default().fg(th.error()), "disconnected".into()),
    };
    let name = if app.self_name.is_empty() {
        "you".to_string()
    } else {
        app.self_name.clone()
    };
    let line = Line::from(vec![
        Span::styled(
            " tox-tui ",
            Style::default().fg(th.header_fg()).bg(th.header_bg()).bold(),
        ),
        Span::raw("  "),
        Span::styled(name, Style::default().add_modifier(Modifier::BOLD)),
        Span::raw("  "),
        Span::styled(conn_text, conn_style),
        Span::raw("  "),
        Span::styled(
            format!("id:{}…", &app.identity_address[..8.min(app.identity_address.len())]),
            Style::default().fg(th.offline()),
        ),
        Span::raw("  "),
        Span::styled(
            format!("[{}]", app.theme.name),
            Style::default().fg(th.accent()),
        ),
    ]);
    f.render_widget(Paragraph::new(line), area);
}

fn draw_body(f: &mut Frame, app: &App, area: Rect) {
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(28), Constraint::Min(20)])
        .split(area);

    draw_sidebar(f, app, cols[0]);
    draw_chat(f, app, cols[1]);
}

fn draw_sidebar(f: &mut Frame, app: &App, area: Rect) {
    let th = &app.theme;
    let border = if app.focus == Focus::Sidebar {
        Style::default().fg(th.accent())
    } else {
        Style::default().fg(th.accent_dim())
    };

    let mut items: Vec<ListItem> = Vec::new();

    // pending requests
    if !app.requests.is_empty() {
        items.push(ListItem::new(Line::from(Span::styled(
            "── requests ──",
            Style::default().fg(th.warn()).bold(),
        ))));
        for (i, r) in app.requests.iter().enumerate() {
            let selected = i == app.selected_request && app.selected_friend == 0;
            let style = if selected {
                Style::default().bg(th.selected_bg()).fg(th.warn())
            } else {
                Style::default().fg(th.warn())
            };
            let label = format!(" {} {}…", if selected { "▶" } else { " " }, &r.public_key[..8]);
            items.push(ListItem::new(Line::from(Span::styled(label, style))));
        }
        items.push(ListItem::new(Line::from("")));
    }

    // friends
    items.push(ListItem::new(Line::from(Span::styled(
        "── friends ──",
        Style::default().fg(th.accent()).bold(),
    ))));
    if app.friends.is_empty() {
        items.push(ListItem::new(Line::from(Span::styled(
            "  (none — press a to add)",
            Style::default().fg(th.offline()),
        ))));
    }
    for (i, fr) in app.friends.iter().enumerate() {
        let selected = i == app.selected_friend && app.selected_request == 0;
        let dot = if fr.connection.is_online() { "●" } else { "○" };
        let dot_color = if fr.connection.is_online() {
            th.online()
        } else {
            th.offline()
        };
        // hotkey hint for friends 1–9
        let hotkey = if i < 9 {
            format!("{}", i + 1)
        } else {
            " ".to_string()
        };
        let mut spans = vec![Span::styled(
            format!(
                "{}{hotkey} {dot} ",
                if selected { "▶" } else { " " }
            ),
            Style::default().fg(dot_color),
        )];
        let name_style = if selected {
            Style::default()
                .bg(th.selected_bg())
                .fg(th.selected_fg())
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };
        spans.push(Span::styled(fr.display_name().to_string(), name_style));
        if fr.unread > 0 {
            spans.push(Span::styled(
                format!(" ({})", fr.unread),
                Style::default().fg(th.warn()).bold(),
            ));
        }
        if fr.typing {
            spans.push(Span::styled(" …", Style::default().fg(th.warn())));
        }
        items.push(ListItem::new(Line::from(spans)));
    }

    // transfers
    if !app.transfers.is_empty() {
        items.push(ListItem::new(Line::from("")));
        items.push(ListItem::new(Line::from(Span::styled(
            "── transfers ──",
            Style::default().fg(th.accent()).bold(),
        ))));
        for t in &app.transfers {
            let dir = if t.incoming { "↓ recv" } else { "↑ send" };
            let pct = if t.size > 0 {
                format!("{} B", t.size)
            } else {
                "stream".into()
            };
            items.push(ListItem::new(Line::from(Span::styled(
                format!("  {dir} {} ({pct})", t.filename),
                Style::default().fg(th.accent()),
            ))));
        }
    }

    let list = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(border)
            .title(" Friends "),
    );
    f.render_widget(list, area);
}

fn draw_chat(f: &mut Frame, app: &App, area: Rect) {
    let th = &app.theme;
    let border = if app.focus == Focus::Chat || app.focus == Focus::Input {
        Style::default().fg(th.accent())
    } else {
        Style::default().fg(th.accent_dim())
    };

    let title = match app.current_friend() {
        Some(fr) => {
            let badge = if fr.unread > 0 {
                format!(" ({})", fr.unread)
            } else {
                String::new()
            };
            format!(
                " {} [{}]{} ",
                fr.display_name(),
                fr.connection.label(),
                badge
            )
        }
        None => " Chat ".to_string(),
    };

    let mut lines: Vec<Line> = Vec::new();

    if app.current_friend().is_none() {
        lines.push(Line::from(Span::styled(
            "Select a friend, or press 'a' to add one.",
            Style::default().fg(th.offline()),
        )));
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "1–9 jump · n/p next/prev · c copy ID · t theme · F1 help",
            Style::default().fg(th.log()),
        )));
    } else {
        // scroll_offset counts up from the bottom
        let total = app.chat.len();
        let visible = area.height.saturating_sub(2) as usize;
        let end = total.saturating_sub(app.scroll_offset);
        let start = end.saturating_sub(visible.max(1));
        for m in &app.chat[start..end] {
            let style = if m.outgoing {
                Style::default().fg(th.outgoing())
            } else {
                Style::default().fg(th.incoming())
            };
            let prefix = if m.outgoing { "you" } else { "peer" };
            lines.push(Line::from(vec![
                Span::styled(format!("[{}] ", m.time), Style::default().fg(th.log())),
                Span::styled(format!("{prefix}: "), style.add_modifier(Modifier::BOLD)),
                Span::styled(m.text.clone(), style),
            ]));
        }
        if app.scroll_offset > 0 {
            lines.push(Line::from(Span::styled(
                format!("── scrolled {} lines up ──", app.scroll_offset),
                Style::default().fg(th.log()),
            )));
        }
    }

    // log under the chat when no friend is selected
    if app.current_friend().is_none() {
        lines.push(Line::from(Span::styled(
            "── log ──",
            Style::default().fg(th.log()),
        )));
        let take = 8.min(app.log.len());
        for entry in &app.log[app.log.len() - take..] {
            let color = if entry.contains("error") || entry.contains("failed") {
                th.error()
            } else {
                th.system()
            };
            lines.push(Line::from(Span::styled(
                format!("  {entry}"),
                Style::default().fg(color),
            )));
        }
    }

    let para = Paragraph::new(lines)
        .wrap(Wrap { trim: false })
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(border)
                .title(title),
        );
    f.render_widget(para, area);
}

fn draw_input(f: &mut Frame, app: &App, area: Rect) {
    let th = &app.theme;
    let (border, title) = match app.focus {
        Focus::Input => (Style::default().fg(th.accent()), " message "),
        _ => (
            Style::default().fg(th.accent_dim()),
            " message (Enter to focus) ",
        ),
    };
    let placeholder = if app.input.is_empty() {
        if app.current_friend().is_some() {
            "type a message…  (/help for commands)"
        } else {
            "/add <Tox ID>  ·  /help"
        }
    } else {
        ""
    };
    let text = if app.input.is_empty() {
        Span::styled(placeholder, Style::default().fg(th.offline()))
    } else {
        Span::raw(app.input.clone())
    };
    let para = Paragraph::new(Line::from(text)).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(border)
            .title(title),
    );
    f.render_widget(para, area);
}

fn draw_status(f: &mut Frame, app: &App, area: Rect) {
    let th = &app.theme;
    let friends_on = app
        .friends
        .iter()
        .filter(|f| f.connection.is_online())
        .count();
    let unread: usize = app.friends.iter().map(|f| f.unread).sum();
    let help = "F1 help · Tab cycle · 1–9 friend · n/p next · c ID · t theme";
    let mut spans = vec![
        Span::styled(
            format!(" friends:{}/{} ", friends_on, app.friends.len()),
            Style::default().fg(th.log()),
        ),
        Span::styled(
            format!(" requests:{} ", app.requests.len()),
            Style::default().fg(if app.requests.is_empty() {
                th.log()
            } else {
                th.warn()
            }),
        ),
    ];
    if unread > 0 {
        spans.push(Span::styled(
            format!(" unread:{unread} "),
            Style::default().fg(th.warn()).bold(),
        ));
    }
    spans.push(Span::raw("  "));
    spans.push(Span::styled(help, Style::default().fg(th.log())));
    f.render_widget(Paragraph::new(Line::from(spans)), area);
}

// dialogs

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

fn draw_help(f: &mut Frame, app: &App) {
    let area = centered_rect(74, 88, f.area());
    let th = &app.theme;
    let text = vec![
        Line::from(Span::styled("tox-tui keybindings", Style::default().bold())),
        Line::from(""),
        Line::from(Span::styled("Global (outside input)", Style::default().fg(th.accent()))),
        Line::from("  Ctrl+Q / Ctrl+C   quit"),
        Line::from("  F1 / ?            toggle this help"),
        Line::from("  Tab               cycle focus: sidebar → chat → input"),
        Line::from("  1–9, 0            jump to friend # (0 = #10)"),
        Line::from("  n / ]             next friend"),
        Line::from("  p / [             previous friend"),
        Line::from("  c                 copy your Tox ID (dialog + clipboard)"),
        Line::from("  t                 theme picker"),
        Line::from("  a                 add friend"),
        Line::from("  i                 set nickname"),
        Line::from("  s                 set status message"),
        Line::from("  f                 send file to current friend"),
        Line::from("  /                 focus input with slash"),
        Line::from("  Esc               close dialog / clear input"),
        Line::from(""),
        Line::from(Span::styled("Sidebar", Style::default().fg(th.accent()))),
        Line::from("  ↑↓ / j k          move selection (requests then friends)"),
        Line::from("  Enter             accept request / focus input on friend"),
        Line::from("  r                 decline selected friend request"),
        Line::from("  Mouse click       select row · scroll wheel navigates"),
        Line::from(""),
        Line::from(Span::styled("Chat", Style::default().fg(th.accent()))),
        Line::from("  ↑↓ / j k          scroll message history"),
        Line::from("  PgUp / PgDn       scroll faster"),
        Line::from("  g / Home          scroll to oldest"),
        Line::from("  G / End           scroll to newest"),
        Line::from("  Enter / i         focus input"),
        Line::from("  Mouse wheel       scroll"),
        Line::from(""),
        Line::from(Span::styled("Slash commands", Style::default().fg(th.accent()))),
        Line::from("  /add <ToxID> [msg]  send friend request"),
        Line::from("  /file <path>        send file to current friend"),
        Line::from("  /me <text>          action message"),
        Line::from("  /name <nick>        set nickname"),
        Line::from("  /status <text>      set status message"),
        Line::from("  /id                 show + copy your Tox ID"),
        Line::from("  /copy               copy Tox ID to clipboard"),
        Line::from("  /theme [name]       picker, or apply theme"),
        Line::from("  /themes             list available themes"),
        Line::from("  /clear              clear visible chat"),
        Line::from("  /help               this help"),
        Line::from("  /quit               exit"),
        Line::from(""),
        Line::from(Span::styled(
            "press any key to close",
            Style::default().fg(th.log()),
        )),
    ];
    let para = Paragraph::new(text).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Help ")
            .border_style(Style::default().fg(th.accent())),
    );
    f.render_widget(Clear, area);
    f.render_widget(para, area);
}

fn draw_add_friend(f: &mut Frame, app: &App) {
    let area = centered_rect(70, 30, f.area());
    let th = &app.theme;
    let title = if app.dialog_step == 0 {
        " Add friend — Tox ID (76 hex) "
    } else {
        " Add friend — greeting message "
    };
    let field = if app.dialog_step == 0 {
        &app.dialog_input
    } else {
        &app.dialog_input2
    };
    let mut lines = vec![Line::from(format!(
        "Enter {}",
        if app.dialog_step == 0 { "Tox ID" } else { "message" }
    ))];
    lines.push(Line::from(""));
    let display = if field.is_empty() { "…" } else { field };
    lines.push(Line::from(Span::styled(
        display.to_string(),
        Style::default().fg(th.outgoing()),
    )));
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "Enter next · Esc cancel · ↑↓ switch field",
        Style::default().fg(th.log()),
    )));
    let para = Paragraph::new(lines).block(
        Block::default()
            .borders(Borders::ALL)
            .title(title)
            .border_style(Style::default().fg(th.warn())),
    );
    f.render_widget(Clear, area);
    f.render_widget(para, area);
}

fn draw_send_file(f: &mut Frame, app: &App) {
    let area = centered_rect(70, 25, f.area());
    let th = &app.theme;
    let text = vec![
        Line::from("Path of file to send:"),
        Line::from(""),
        Line::from(if app.dialog_input.is_empty() {
            "…".to_string()
        } else {
            app.dialog_input.clone()
        }),
        Line::from(""),
        Line::from(Span::styled(
            "Enter send · Esc cancel",
            Style::default().fg(th.log()),
        )),
    ];
    let para = Paragraph::new(text).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Send file ")
            .border_style(Style::default().fg(th.accent())),
    );
    f.render_widget(Clear, area);
    f.render_widget(para, area);
}

fn draw_incoming_file(f: &mut Frame, app: &App) {
    let area = centered_rect(60, 30, f.area());
    let th = &app.theme;
    let t = app.transfers.get(app.selected_transfer);
    let (name, size, from) = match t {
        Some(t) => (
            t.filename.clone(),
            t.size,
            app.friends
                .iter()
                .find(|f| f.number == t.friend_number)
                .map(|f| f.display_name().to_string())
                .unwrap_or_else(|| format!("#{}", t.friend_number)),
        ),
        None => ("?".into(), 0, "?".into()),
    };
    let text = vec![
        Line::from(Span::styled(
            "Incoming file transfer",
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(format!("From:  {from}")),
        Line::from(format!("File:  {name}")),
        Line::from(format!("Size:  {size} bytes")),
        Line::from(""),
        Line::from(Span::styled(
            "Accept?  [y]es   [n]o",
            Style::default().fg(th.warn()).add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(
            "Saved under ~/Downloads/tox-downloads/",
            Style::default().fg(th.log()),
        )),
    ];
    let para = Paragraph::new(text).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Receive file ")
            .border_style(Style::default().fg(th.accent())),
    );
    f.render_widget(Clear, area);
    f.render_widget(para, area);
}

fn draw_friend_request(f: &mut Frame, app: &App) {
    let area = centered_rect(60, 35, f.area());
    let th = &app.theme;
    let r = app.requests.get(app.selected_request);
    let (pk, msg) = match r {
        Some(r) => (r.public_key.clone(), r.message.clone()),
        None => ("?".into(), "?".into()),
    };
    let text = vec![
        Line::from(Span::styled(
            "Friend request",
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(format!("From: {pk}")),
        Line::from(""),
        Line::from(format!("“{msg}”")),
        Line::from(""),
        Line::from(Span::styled(
            "[y] accept   [n] decline",
            Style::default().fg(th.warn()),
        )),
    ];
    let para = Paragraph::new(text).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Friend request ")
            .border_style(Style::default().fg(th.warn())),
    );
    f.render_widget(Clear, area);
    f.render_widget(para, area);
}

fn draw_prompt(f: &mut Frame, app: &App, title: &str, input: &str) {
    let area = centered_rect(60, 20, f.area());
    let th = &app.theme;
    let text = vec![
        Line::from(format!("{title}:")),
        Line::from(""),
        Line::from(if input.is_empty() {
            "…".to_string()
        } else {
            input.to_string()
        }),
        Line::from(""),
        Line::from(Span::styled(
            "Enter save · Esc cancel",
            Style::default().fg(th.log()),
        )),
    ];
    let para = Paragraph::new(text).block(
        Block::default()
            .borders(Borders::ALL)
            .title(format!(" {title} "))
            .border_style(Style::default().fg(th.accent())),
    );
    f.render_widget(Clear, area);
    f.render_widget(para, area);
}

// full id, wrapped for easy selection; auto-copied on open
fn draw_show_id(f: &mut Frame, app: &App) {
    let area = centered_rect(80, 40, f.area());
    let th = &app.theme;
    let id = &app.identity_address;
    // break the 76-char id into two lines
    let chunks: Vec<&str> = id.as_bytes().chunks(38).map(|c| std::str::from_utf8(c).unwrap_or("")).collect();
    let mut lines = vec![
        Line::from(Span::styled(
            "Your Tox ID",
            Style::default().bold().fg(th.accent()),
        )),
        Line::from(""),
    ];
    for c in &chunks {
        lines.push(Line::from(Span::styled(
            c.to_string(),
            Style::default().fg(th.outgoing()),
        )));
    }
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        app.clipboard_status.clone(),
        Style::default().fg(th.online()),
    )));
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "c / y re-copy · Enter copy+close · Esc close",
        Style::default().fg(th.log()),
    )));
    let para = Paragraph::new(lines).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Your Tox ID ")
            .border_style(Style::default().fg(th.accent())),
    );
    f.render_widget(Clear, area);
    f.render_widget(para, area);
}

fn draw_theme_picker(f: &mut Frame, app: &App) {
    let area = centered_rect(50, 60, f.area());
    let th = &app.theme;
    let names = app.theme_names();
    let mut lines = vec![Line::from(Span::styled(
        "Select a theme (Enter / number, Esc cancel)",
        Style::default().fg(th.log()),
    )),
        Line::from(""),
    ];
    for (i, name) in names.iter().enumerate() {
        let selected = i == app.selected_theme;
        let marker = if selected { "▶ " } else { "  " };
        let style = if selected {
            Style::default()
                .bg(th.selected_bg())
                .fg(th.selected_fg())
                .bold()
        } else if name == &app.theme.name {
            Style::default().fg(th.accent())
        } else {
            Style::default().fg(th.outgoing())
        };
        // Show hotkey numbers for first 9 entries.
        let hot = if i < 9 {
            format!("{} ", i + 1)
        } else {
            "  ".to_string()
        };
        lines.push(Line::from(Span::styled(
            format!("{marker}{hot}{name}"),
            style,
        )));
    }
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        format!("custom file: {}", app.themes.path().display()),
        Style::default().fg(th.log()),
    )));
    let para = Paragraph::new(lines).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Themes ")
            .border_style(Style::default().fg(th.accent())),
    );
    f.render_widget(Clear, area);
    f.render_widget(para, area);
}
