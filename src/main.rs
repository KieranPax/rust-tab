extern crate crossterm;
mod app;
mod error;
mod tab;
mod window;

use error::Result;

fn main() -> Result<()> {
    app::App::new()?.run()
}
