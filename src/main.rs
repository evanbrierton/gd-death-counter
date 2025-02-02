use std::path::PathBuf;

use anyhow::Ok;
use clap::Parser;
use gd_death_counter::watch::DataWatcher;

#[derive(Parser, Debug)]
struct Args {
    path: PathBuf,
    output: Option<PathBuf>,

    #[arg(short, long, value_enum, default_value = "0")]
    baseline: u32,

    #[arg(short, long, default_value = "30000")]
    interval: u64,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    let mut watcher = DataWatcher::new(args.baseline, args.interval, args.path, args.output);

    watcher.watch()?;

    Ok(())
}
