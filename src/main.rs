extern crate crossterm;
extern crate tui;
mod app;
mod error;
mod tab;
mod window;

use error::Result;

fn main() -> Result<()> {
    let mut win = window::Window::new()?;
    win.test()?;
    win.close()
}
