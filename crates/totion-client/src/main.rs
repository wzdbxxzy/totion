mod api;
mod app;
mod charts;
mod config;
mod ime;
mod ui;

use std::io;
use std::time::Duration;

use anyhow::Result;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};

fn main() -> Result<()> {
    if std::env::args().any(|arg| arg == "--version" || arg == "-V") {
        println!("totion {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }
    if std::env::args().any(|arg| arg == "--help" || arg == "-h") {
        println!(
            "totion {} — 终端共享空间\n\n用法: totion\n\n首次启动在界面中填写服务器地址、名字与邀请码。",
            env!("CARGO_PKG_VERSION")
        );
        return Ok(());
    }

    let mut app = app::App::boot();

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let result = run(&mut terminal, &mut app);

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        DisableMouseCapture,
        LeaveAlternateScreen
    )?;
    terminal.show_cursor()?;
    ime::set_enabled(true);

    result
}

fn run(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>, app: &mut app::App) -> Result<()> {
    let mut ime_enabled = app.needs_ime();
    ime::set_enabled(ime_enabled);
    loop {
        terminal.draw(|frame| ui::draw(frame, app))?;
        if event::poll(Duration::from_millis(200))? {
            match event::read()? {
                Event::Key(key) => app.handle_key(key),
                Event::Mouse(mouse) => app.handle_mouse(mouse),
                _ => {}
            }
            let needs_ime = app.needs_ime();
            if needs_ime != ime_enabled {
                ime_enabled = needs_ime;
                ime::set_enabled(ime_enabled);
            }
        }
        app.tick();
        if app.should_quit {
            break;
        }
    }
    Ok(())
}
