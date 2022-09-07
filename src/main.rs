extern crate crossterm;
mod app;
mod error;
mod window;

use error::Result;

fn main() -> Result<()> {
    app::App::new()?.run()
}
