use clap::Parser;
use gd_death_counter::watch::DataWatcher;
use std::path::PathBuf;

#[derive(Parser, Debug)]
struct Args {
    path: PathBuf,
    output: Option<PathBuf>,

    #[arg(short, long, value_enum, default_value = "0")]
    baseline: u32,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    DataWatcher::new(args.baseline, args.path, args.output).watch()
}
