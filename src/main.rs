extern crate clap;
extern crate crossterm;
extern crate fraction;
extern crate serde;
extern crate serde_json;
extern crate serde_repr;
mod app;
mod dur;
mod error;
mod song;
mod window;

use error::Result;

fn main() -> Result<()> {
    app::App::new()?.run()
}
