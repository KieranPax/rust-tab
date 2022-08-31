use crate::{error::Result, map_io_err};
use std::io;

pub struct Window {
    terminal: tui::terminal::Terminal<tui::backend::CrosstermBackend<io::Stdout>>,
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
        let backend = tui::backend::CrosstermBackend::new(stdout);
        Ok(Self {
            terminal: map_io_err!(tui::Terminal::new(backend))?,
        })
    }

    pub fn test(&mut self) -> Result<()> {
        map_io_err!(self.terminal.draw(|f| {
            let size = f.size();
            let block = tui::widgets::Block::default()
                .title("Block")
                .borders(tui::widgets::Borders::ALL);
            f.render_widget(block, size);
        }))?;
        std::thread::sleep(std::time::Duration::from_millis(1000));
        Ok(())
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
}
