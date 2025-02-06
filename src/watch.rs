use std::{
    collections::HashMap,
    fs::{self, File},
    io::{BufReader, Seek, Write},
    path::{self, PathBuf},
    result::Result::Ok,
    sync::mpsc::channel,
};

use notify::{
    event::{ModifyKind, RenameMode},
    recommended_watcher, Event, EventKind, RecursiveMode, Watcher,
};

use crate::level::Level;

pub struct DataWatcher {
    baseline: u32,
    input: PathBuf,
    output: Option<File>,
    deaths: HashMap<PathBuf, u32>,
    open_files: HashMap<PathBuf, std::io::Result<BufReader<File>>>,
    previous: Option<u32>,
}

impl DataWatcher {
    pub fn new(baseline: u32, input: PathBuf, output_path: Option<PathBuf>) -> Self {
        Self {
            baseline,
            input,
            output: match output_path {
                Some(o) => match File::create(o) {
                    Ok(file) => Some(file),
                    Err(_) => None,
                },
                None => None,
            },
            deaths: HashMap::new(),
            open_files: HashMap::new(),
            previous: None,
        }
    }

    fn write_deaths(&mut self, deaths: u32) -> anyhow::Result<()> {
        if let Some(previous) = self.previous {
            if deaths == previous {
                return Ok(());
            }
        }

        match self.output {
            Some(ref mut output) => write!(output, "{}", deaths)?,
            None => println!("{}", deaths),
        };

        self.previous = Some(deaths);
        Ok(())
    }

    fn remove_files(&mut self, paths: &Vec<PathBuf>) {
        for path in paths {
            self.deaths.remove(path);
            self.open_files.remove(path);
        }
    }

    fn get_total_deaths(&self) -> u32 {
        self.deaths.values().sum::<u32>() + self.baseline
    }

    fn compute_deaths_file(reader: &mut BufReader<File>) -> anyhow::Result<u32> {
        reader.rewind()?;
        let level: Level = serde_json::from_reader(reader)?;
        Ok(level.total_deaths())
    }

    fn compute_deaths(&mut self, paths: &Vec<PathBuf>) -> anyhow::Result<()> {
        for path in paths {
            if path.extension().map_or(false, |ext| ext == "json") {
                let reader = self
                    .open_files
                    .entry(path.to_owned())
                    .or_insert_with(|| File::open(path).map(BufReader::new));

                if let Ok(reader) = reader.as_mut() {
                    if let Ok(deaths) = Self::compute_deaths_file(reader) {
                        self.deaths.insert(path.to_owned(), deaths);
                    }
                }
            }
        }

        Ok(())
    }

    fn compute_all_deaths(&mut self) -> anyhow::Result<()> {
        self.deaths.clear();
        let directory = fs::read_dir(&self.input)?;

        self.compute_deaths(
            &directory
                .filter_map(|entry| entry.ok().map(|entry| entry.path()))
                .map(path::absolute)
                .filter_map(Result::ok)
                .collect(),
        )
    }

    fn handle(&mut self, event: &Event) -> anyhow::Result<()> {
        match event.kind {
            EventKind::Modify(ModifyKind::Name(RenameMode::Any)) => self.compute_all_deaths()?,
            EventKind::Remove(_) => self.remove_files(&event.paths),
            EventKind::Create(_) | EventKind::Modify(_) => self.compute_deaths(&event.paths)?,
            _ => return Ok(()),
        }

        self.write_deaths(self.get_total_deaths())
    }

    pub fn watch(&mut self) -> anyhow::Result<()> {
        self.compute_all_deaths()?;
        self.write_deaths(self.get_total_deaths())?;

        let (tx, rx) = channel();

        let mut watcher = recommended_watcher(tx)?;

        watcher.watch(&self.input, RecursiveMode::NonRecursive)?;

        for result in rx {
            self.handle(&result?)?;
        }

        Ok(())
    }
}
