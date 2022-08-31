use crate::{error::Result, window};

pub struct App {
    win: window::Window,
}

impl App {
    pub fn new() -> Result<Self> {
        Ok(Self {
            win: window::Window::new()?,
        })
    }

    pub fn draw(&mut self) -> Result<()> {
        self.win.draw(|f| {
            let size = f.size();
            let block = tui::widgets::Block::default()
                .title("Window")
                .borders(tui::widgets::Borders::ALL);
            f.render_widget(block, size);
        })?;
        Ok(())
    }

    pub fn run(mut self) -> Result<()> {
        self.draw()?;
        std::thread::sleep(std::time::Duration::from_millis(1000));
        self.win.close()
    }
}
