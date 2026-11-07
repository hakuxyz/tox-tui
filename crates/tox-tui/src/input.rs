// keyboard / mouse handling

use crate::app::{App, Dialog, Focus};
use anyhow::Result;
use crossterm::event::{
    Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseEvent, MouseEventKind,
};

// returns true when the app should quit
pub async fn handle_event(app: &mut App, ev: Event) -> Result<bool> {
    match ev {
        Event::Key(key) if key.kind == KeyEventKind::Press => handle_key(app, key).await,
        Event::Mouse(mouse) => {
            handle_mouse(app, mouse);
            Ok(app.should_quit)
        }
        _ => Ok(app.should_quit),
    }
}

async fn handle_key(app: &mut App, key: KeyEvent) -> Result<bool> {
    // ctrl+q / ctrl+c
    if key.modifiers.contains(KeyModifiers::CONTROL) {
        match key.code {
            KeyCode::Char('q') => {
                app.should_quit = true;
                return Ok(true);
            }
            KeyCode::Char('c') => {
                app.should_quit = true;
                return Ok(true);
            }
            _ => {}
        }
    }

    if app.dialog != Dialog::None && app.dialog != Dialog::Help {
        return handle_dialog_key(app, key).await;
    }
    if app.dialog == Dialog::Help {
        // Any key closes help
        if matches!(
            key.code,
            KeyCode::Esc | KeyCode::Enter | KeyCode::Char(_) | KeyCode::F(1)
        ) {
            app.dialog = Dialog::None;
        }
        return Ok(app.should_quit);
    }

    // nav keys only outside the input box
    if app.focus != Focus::Input {
        if let Some(handled) = global_nav_key(app, key)? {
            if handled {
                return Ok(true);
            }
            return Ok(app.should_quit);
        }
    }

    match app.focus {
        Focus::Sidebar => handle_sidebar_key(app, key).await,
        Focus::Chat => handle_chat_key(app, key).await,
        Focus::Input => handle_input_key(app, key).await,
    }
}

// keys shared by sidebar/chat:
// Some(true)=quit, Some(false)=handled, None=fall through
fn global_nav_key(app: &mut App, key: KeyEvent) -> Result<Option<bool>> {
    match key.code {
        // copy tox id
        KeyCode::Char('c') => {
            app.open_id_dialog();
            Ok(Some(false))
        }
        // theme picker
        KeyCode::Char('t') => {
            app.dialog = Dialog::ThemePicker;
            let names = app.theme_names();
            app.selected_theme = names.iter().position(|n| n == &app.theme.name).unwrap_or(0);
            Ok(Some(false))
        }
        // 1–9 jump to friend
        KeyCode::Char(c) if c.is_ascii_digit() && c != '0' => {
            let n = c as usize - '0' as usize;
            app.jump_to_friend(n);
            Ok(Some(false))
        }
        // 0 → friend #10
        KeyCode::Char('0') => {
            app.jump_to_friend(10);
            Ok(Some(false))
        }
        // next / prev friend
        KeyCode::Char('n') | KeyCode::Char(']') => {
            app.next_friend();
            Ok(Some(false))
        }
        KeyCode::Char('p') | KeyCode::Char('[') => {
            app.prev_friend();
            Ok(Some(false))
        }
        // tab cycles focus
        KeyCode::Tab => {
            app.focus = match app.focus {
                Focus::Sidebar => Focus::Chat,
                Focus::Chat => Focus::Input,
                Focus::Input => Focus::Sidebar,
            };
            Ok(Some(false))
        }
        // help from anywhere
        KeyCode::F(1) | KeyCode::Char('?') => {
            app.dialog = Dialog::Help;
            Ok(Some(false))
        }
        // quick actions
        KeyCode::Char('a') => {
            app.dialog = Dialog::AddFriend;
            app.dialog_input.clear();
            app.dialog_input2.clear();
            app.dialog_step = 0;
            app.focus = Focus::Input;
            Ok(Some(false))
        }
        KeyCode::Char('i') => {
            app.dialog = Dialog::SetName;
            app.dialog_input = app.self_name.clone();
            app.focus = Focus::Input;
            Ok(Some(false))
        }
        KeyCode::Char('s') => {
            app.dialog = Dialog::SetStatus;
            app.dialog_input.clear();
            app.focus = Focus::Input;
            Ok(Some(false))
        }
        KeyCode::Char('f') => {
            if app.current_friend().is_some() {
                app.dialog = Dialog::SendFile;
                app.dialog_input.clear();
                app.focus = Focus::Input;
            } else {
                app.log.push("select a friend before sending a file".into());
            }
            Ok(Some(false))
        }
        KeyCode::Char('/') => {
            app.focus = Focus::Input;
            app.input.push('/');
            Ok(Some(false))
        }
        _ => Ok(None),
    }
}

async fn handle_sidebar_key(app: &mut App, key: KeyEvent) -> Result<bool> {
    match key.code {
        KeyCode::Tab | KeyCode::Right | KeyCode::Char('l') => {
            app.focus = Focus::Chat;
        }
        KeyCode::Down | KeyCode::Char('j') => {
            // Cycle through: requests first (if any), then friends
            let total = app.requests.len() + app.friends.len();
            if total == 0 {
                return Ok(app.should_quit);
            }
            let sel = app.selected_request + app.selected_friend;
            let next = (sel + 1) % total;
            if next < app.requests.len() {
                app.selected_request = next;
                app.selected_friend = 0;
                app.chat.clear();
            } else {
                app.selected_request = 0;
                app.select_friend(next - app.requests.len());
            }
        }
        KeyCode::Up | KeyCode::Char('k') => {
            let total = app.requests.len() + app.friends.len();
            if total == 0 {
                return Ok(app.should_quit);
            }
            let sel = app.selected_request + app.selected_friend;
            let next = (sel + total - 1) % total;
            if next < app.requests.len() {
                app.selected_request = next;
                app.selected_friend = 0;
                app.chat.clear();
            } else {
                app.selected_request = 0;
                app.select_friend(next - app.requests.len());
            }
        }
        KeyCode::Enter => {
            if !app.requests.is_empty()
                && app.selected_friend == 0
                && app.selected_request < app.requests.len()
            {
                // on a request row → accept
                app.accept_selected_request()?;
            } else if app.current_friend().is_some() {
                // on a friend row → jump to typing
                app.focus = Focus::Input;
            }
        }
        KeyCode::Char('r') => {
            app.decline_selected_request()?;
        }
        _ => {}
    }
    Ok(app.should_quit)
}

async fn handle_chat_key(app: &mut App, key: KeyEvent) -> Result<bool> {
    match key.code {
        KeyCode::Left | KeyCode::Char('h') => {
            app.focus = Focus::Sidebar;
        }
        KeyCode::Down | KeyCode::Char('j') => {
            let max = app.chat.len().saturating_sub(1);
            if app.scroll_offset < max {
                app.scroll_offset += 1;
            }
        }
        KeyCode::Up | KeyCode::Char('k') => {
            app.scroll_offset = app.scroll_offset.saturating_sub(1);
        }
        KeyCode::PageDown => {
            let max = app.chat.len().saturating_sub(1);
            app.scroll_offset = (app.scroll_offset + 10).min(max);
        }
        KeyCode::PageUp => {
            app.scroll_offset = app.scroll_offset.saturating_sub(10);
        }
        KeyCode::Home | KeyCode::Char('g') => {
            app.scroll_offset = app.chat.len().saturating_sub(1);
        }
        KeyCode::End | KeyCode::Char('G') => {
            app.scroll_offset = 0;
        }
        KeyCode::Enter | KeyCode::Char('i') => {
            app.focus = Focus::Input;
        }
        _ => {}
    }
    Ok(app.should_quit)
}

async fn handle_input_key(app: &mut App, key: KeyEvent) -> Result<bool> {
    match key.code {
        KeyCode::Esc => {
            app.input.clear();
            app.focus = Focus::Sidebar;
        }
        KeyCode::Tab => {
            app.focus = Focus::Sidebar;
        }
        KeyCode::Enter => {
            let text = std::mem::take(&mut app.input);
            app.send_chat(&text)?;
        }
        KeyCode::Backspace => {
            app.input.pop();
        }
        KeyCode::Delete => {
            app.input.clear();
        }
        KeyCode::Up => {
            // reserved for message history
        }
        KeyCode::Down => {}
        KeyCode::Char(c) => {
            app.input.push(c);
        }
        _ => {}
    }
    Ok(app.should_quit)
}

async fn handle_dialog_key(app: &mut App, key: KeyEvent) -> Result<bool> {
    // esc closes any dialog
    if matches!(key.code, KeyCode::Esc) {
        app.dialog = Dialog::None;
        app.dialog_input.clear();
        app.dialog_input2.clear();
        app.dialog_step = 0;
        app.focus = Focus::Sidebar;
        return Ok(app.should_quit);
    }

    match app.dialog.clone() {
        Dialog::ShowId => match key.code {
            KeyCode::Char('c') | KeyCode::Char('y') | KeyCode::Enter => {
                app.copy_id();
                // Enter closes after copy; c/y keep it open to re-copy.
                if matches!(key.code, KeyCode::Enter) {
                    app.dialog = Dialog::None;
                }
            }
            _ => {}
        },
        Dialog::ThemePicker => {
            let names = app.theme_names();
            match key.code {
                KeyCode::Down | KeyCode::Char('j') => {
                    if !names.is_empty() {
                        app.selected_theme = (app.selected_theme + 1) % names.len();
                    }
                }
                KeyCode::Up | KeyCode::Char('k') => {
                    if !names.is_empty() {
                        app.selected_theme = (app.selected_theme + names.len() - 1) % names.len();
                    }
                }
                KeyCode::Enter => {
                    if let Some(name) = names.get(app.selected_theme).cloned() {
                        app.apply_theme(&name);
                    }
                    app.dialog = Dialog::None;
                }
                // number jump in picker
                KeyCode::Char(c) if c.is_ascii_digit() && c != '0' => {
                    let n = c as usize - '0' as usize;
                    if let Some(name) = names.get(n - 1).cloned() {
                        app.apply_theme(&name);
                        app.dialog = Dialog::None;
                    }
                }
                _ => {}
            }
        }
        Dialog::AddFriend => match key.code {
            KeyCode::Enter => {
                if app.dialog_step == 0 {
                    if app.dialog_input.trim().is_empty() {
                        return Ok(app.should_quit);
                    }
                    app.dialog_step = 1;
                } else {
                    let address = app.dialog_input.clone();
                    let message = if app.dialog_input2.trim().is_empty() {
                        "Hi from tox-tui!".into()
                    } else {
                        app.dialog_input2.clone()
                    };
                    app.tox
                        .send(tox_client::ToxCommand::AddFriend { address, message })?;
                    app.dialog = Dialog::None;
                    app.dialog_input.clear();
                    app.dialog_input2.clear();
                    app.dialog_step = 0;
                    app.focus = Focus::Sidebar;
                }
            }
            KeyCode::Backspace => {
                if app.dialog_step == 0 {
                    app.dialog_input.pop();
                } else {
                    app.dialog_input2.pop();
                }
            }
            KeyCode::Up | KeyCode::Down => {
                app.dialog_step = 1 - app.dialog_step;
            }
            KeyCode::Char(c) => {
                if app.dialog_step == 0 {
                    app.dialog_input.push(c);
                } else {
                    app.dialog_input2.push(c);
                }
            }
            _ => {}
        },
        Dialog::SendFile => match key.code {
            KeyCode::Enter => {
                if let (Some(fn_num), Some(path)) =
                    (app.current_friend_number(), Some(app.dialog_input.clone()))
                {
                    if !path.trim().is_empty() {
                        app.tox.send(tox_client::ToxCommand::SendFile(
                            tox_client::FileSendRequest {
                                friend_number: fn_num,
                                path: std::path::PathBuf::from(path.trim()),
                            },
                        ))?;
                    }
                }
                app.dialog = Dialog::None;
                app.dialog_input.clear();
            }
            KeyCode::Backspace => {
                app.dialog_input.pop();
            }
            KeyCode::Char(c) => {
                app.dialog_input.push(c);
            }
            _ => {}
        },
        Dialog::IncomingFile => match key.code {
            KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter => {
                if let Some(t) = app.transfers.get(app.selected_transfer) {
                    let (fn_num, file_num) = (t.friend_number, t.file_number);
                    let dest = dirs::download_dir()
                        .or_else(dirs::home_dir)
                        .unwrap_or_else(|| std::path::PathBuf::from("."))
                        .join("tox-downloads");
                    app.tox.send(tox_client::ToxCommand::AcceptFile {
                        friend_number: fn_num,
                        file_number: file_num,
                        dest_dir: dest,
                    })?;
                }
                app.dialog = Dialog::None;
            }
            KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                if let Some(t) = app.transfers.get(app.selected_transfer) {
                    let (fn_num, file_num) = (t.friend_number, t.file_number);
                    app.tox.send(tox_client::ToxCommand::CancelFile {
                        friend_number: fn_num,
                        file_number: file_num,
                    })?;
                    app.transfers.remove(app.selected_transfer);
                }
                app.dialog = Dialog::None;
            }
            _ => {}
        },
        Dialog::SetName => match key.code {
            KeyCode::Enter => {
                let name = app.dialog_input.clone();
                if !name.trim().is_empty() {
                    app.tox
                        .send(tox_client::ToxCommand::SetName(name.clone()))?;
                    app.self_name = name;
                }
                app.dialog = Dialog::None;
                app.focus = Focus::Sidebar;
            }
            KeyCode::Backspace => {
                app.dialog_input.pop();
            }
            KeyCode::Char(c) => {
                app.dialog_input.push(c);
            }
            _ => {}
        },
        Dialog::SetStatus => match key.code {
            KeyCode::Enter => {
                let sm = app.dialog_input.clone();
                app.tox.send(tox_client::ToxCommand::SetStatusMessage(sm))?;
                app.dialog = Dialog::None;
                app.focus = Focus::Sidebar;
            }
            KeyCode::Backspace => {
                app.dialog_input.pop();
            }
            KeyCode::Char(c) => {
                app.dialog_input.push(c);
            }
            _ => {}
        },
        Dialog::FriendRequest => match key.code {
            KeyCode::Char('y') | KeyCode::Enter => {
                app.accept_selected_request()?;
                app.dialog = Dialog::None;
            }
            KeyCode::Char('n') => {
                app.decline_selected_request()?;
                app.dialog = Dialog::None;
            }
            _ => {}
        },
        _ => {
            app.dialog = Dialog::None;
        }
    }
    Ok(app.should_quit)
}

// scroll + click handling
fn handle_mouse(app: &mut App, mouse: MouseEvent) {
    match mouse.kind {
        MouseEventKind::ScrollDown => {
            match app.focus {
                Focus::Chat => {
                    let max = app.chat.len().saturating_sub(1);
                    app.scroll_offset = (app.scroll_offset + 3).min(max);
                }
                Focus::Sidebar => {
                    // Reuse j/k navigation
                    let total = app.requests.len() + app.friends.len();
                    if total > 0 {
                        let sel = app.selected_request + app.selected_friend;
                        let next = (sel + 1) % total;
                        if next < app.requests.len() {
                            app.selected_request = next;
                            app.selected_friend = 0;
                            app.chat.clear();
                        } else {
                            app.selected_request = 0;
                            app.select_friend(next - app.requests.len());
                        }
                    }
                }
                Focus::Input => {}
            }
        }
        MouseEventKind::ScrollUp => match app.focus {
            Focus::Chat => {
                app.scroll_offset = app.scroll_offset.saturating_sub(3);
            }
            Focus::Sidebar => {
                let total = app.requests.len() + app.friends.len();
                if total > 0 {
                    let sel = app.selected_request + app.selected_friend;
                    let next = (sel + total - 1) % total;
                    if next < app.requests.len() {
                        app.selected_request = next;
                        app.selected_friend = 0;
                        app.chat.clear();
                    } else {
                        app.selected_request = 0;
                        app.select_friend(next - app.requests.len());
                    }
                }
            }
            Focus::Input => {}
        },
        MouseEventKind::Down(_) => {
            // click focuses and selects by row
            // 1-row header, body below; sidebar is 28 cols
            let col = mouse.column;
            let row = mouse.row;
            if row == 0 {
                return; // header
            }
            if col < 28 {
                app.focus = Focus::Sidebar;
                // skip top border of the sidebar block
                let content_row = usize::from(row.saturating_sub(2));
                // same row order as ui::draw_sidebar
                let mut idx: usize = 0;
                let mut hit = None;
                if !app.requests.is_empty() {
                    // line 0: header "── requests ──"
                    idx += 1;
                    for _ in 0..app.requests.len() {
                        if idx == content_row {
                            hit = Some(Hit::Request);
                            // We'll refine below using per-item index.
                            break;
                        }
                        idx += 1;
                    }
                    // blank line
                    if hit.is_none() && idx == content_row {
                        hit = Some(Hit::None);
                    }
                    idx += 1; // blank
                }
                // friends header
                if hit.is_none() {
                    if idx == content_row {
                        hit = Some(Hit::None);
                    }
                    idx += 1;
                }
                if hit.is_none() {
                    for i in 0..app.friends.len() {
                        if idx == content_row {
                            hit = Some(Hit::Friend(i));
                            break;
                        }
                        idx += 1;
                    }
                }
                // which request row was hit
                if let Some(Hit::Request) = hit {
                    // content_row after header → request index
                    let base = if app.requests.is_empty() { 0 } else { 1 };
                    let ridx = content_row.saturating_sub(base);
                    if ridx < app.requests.len() {
                        app.selected_request = ridx;
                        app.selected_friend = 0;
                        app.chat.clear();
                        app.dialog = Dialog::FriendRequest;
                    }
                }
                if let Some(Hit::Friend(i)) = hit {
                    app.select_friend(i);
                }
            } else if row > 0 {
                // click in chat area → focus chat
                app.focus = Focus::Chat;
            }
        }
        MouseEventKind::Moved | MouseEventKind::Drag(_) => {}
        _ => {}
    }
}

enum Hit {
    None,
    Request,
    Friend(usize),
}
