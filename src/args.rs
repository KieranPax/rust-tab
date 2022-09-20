#[derive(clap::Parser, Debug)]
#[clap(name = "rust-tab")]
#[clap(version, about, long_about = None)]
pub struct Args {
    #[clap(value_parser)]
    pub path: Option<String>,
    #[clap(short, long, action)]
    pub draw_timer: bool,
}
