use std::path::PathBuf;

use anyhow::Ok;
use clap::Parser;
use death_counter_rs::watch::{self, DataWatcher};

#[derive(Parser, Debug)]
struct Args {
    path: PathBuf,
    output: Option<PathBuf>,

    #[arg(short, long, value_enum, default_value = "0")]
    baseline: u32,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    let mut watcher = DataWatcher::new(args.baseline, args.path, args.output);

    watcher.watch()?;

    Ok(())
}
