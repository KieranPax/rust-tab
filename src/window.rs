use crate::{error::Result, map_io_err};
use std::io;

type Backend = tui::backend::CrosstermBackend<io::Stdout>;

pub struct Window {
    terminal: tui::terminal::Terminal<Backend>,
}

impl Window {
    pub fn new() -> Result<Self> {
        map_io_err!(crossterm::terminal::enable_raw_mode())?;
        let mut stdout = io::stdout();
        map_io_err!(crossterm::execute!(
            stdout,
            crossterm::terminal::EnterAlternateScreen,
            crossterm::event::EnableMouseCapture
        ))?;
        let backend = Backend::new(stdout);
        Ok(Self {
            terminal: map_io_err!(tui::Terminal::new(backend))?,
        })
    }

    pub fn close(mut self) -> Result<()> {
        map_io_err!(crossterm::terminal::disable_raw_mode())?;
        map_io_err!(crossterm::execute!(
            self.terminal.backend_mut(),
            crossterm::terminal::LeaveAlternateScreen,
            crossterm::event::DisableMouseCapture
        ))?;
        map_io_err!(self.terminal.show_cursor())?;
        Ok(())
    }

    pub fn draw<F>(&mut self, f: F) -> Result<tui::terminal::CompletedFrame>
    where
        F: FnOnce(&mut tui::Frame<Backend>),
    {
        map_io_err!(self.terminal.draw(f))
    }
}
