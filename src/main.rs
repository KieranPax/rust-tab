extern crate clap;
extern crate crossterm;
extern crate fraction;
extern crate lazy_static;
extern crate regex;
extern crate serde;
extern crate serde_json;
extern crate serde_repr;
mod app;
mod buffer;
mod cursor;
mod dur;
mod error;
mod history;
mod song;
mod window;

use error::Result;

fn main() -> Result<()> {
    app::App::new()?.run()
}
