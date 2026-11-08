// tox-tui — terminal tox messenger.

use anyhow::Result;
use crossterm::event::{DisableMouseCapture, EnableMouseCapture};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use futures::StreamExt;
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use std::io;
use std::time::Duration;
use tox_client::ToxConfig;
use tox_tui::app::App;
use tox_tui::{input, ui};

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::Builder::from_env(
        env_logger::Env::default().default_filter_or("tox_tui=info,tox_client=info"),
    )
    .target(env_logger::Target::Stderr)
    .init();

    let config = build_config()?;
    let mut app = App::new(config).await?;

    // Terminal setup
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    terminal.clear()?;

    let res = run_loop(&mut terminal, &mut app).await;

    // Teardown
    let _ = app.shutdown().await;
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    res
}

fn build_config() -> Result<ToxConfig> {
    let mut cfg = ToxConfig::default();
    if let Ok(dir) = std::env::var("TOX_TUI_DATA_DIR") {
        cfg.data_dir = std::path::PathBuf::from(dir);
    }
    if let Ok(name) = std::env::var("TOX_TUI_NAME") {
        cfg.name = Some(name);
    }
    if let Ok(sm) = std::env::var("TOX_TUI_STATUS") {
        cfg.status_message = Some(sm);
    }
    Ok(cfg)
}

async fn run_loop<B: ratatui::backend::Backend>(
    terminal: &mut ratatui::Terminal<B>,
    app: &mut App,
) -> Result<()> {
    let mut events = crossterm::event::EventStream::new();
    let tick = tokio::time::interval(Duration::from_millis(50));
    tokio::pin!(tick);

    loop {
        terminal.draw(|f| ui::draw(f, app))?;

        tokio::select! {
            maybe_ev = events.next() => {
                if let Some(Ok(ev)) = maybe_ev {
                    if input::handle_event(app, ev).await? {
                        break; // quit
                    }
                }
            }
            _ = tick.tick() => {
                app.pump_events();
                if app.should_quit {
                    break;
                }
            }
        }
    }
    Ok(())
}
