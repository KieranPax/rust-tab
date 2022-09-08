extern crate crossterm;
extern crate serde;
extern crate serde_json;
extern crate serde_repr;
mod app;
mod error;
mod window;

use error::Result;

fn main() -> Result<()> {
    app::App::new()?.run()
}
