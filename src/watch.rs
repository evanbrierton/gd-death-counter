use std::{
    collections::HashMap,
    fs::{self, File, ReadDir},
    io::Write,
    path::{Path, PathBuf},
    sync::{
        mpsc::{channel, Receiver},
        Arc, Mutex,
    },
    thread,
    time::Duration,
};

use notify::{
    event::{CreateKind, DataChange, ModifyKind, RemoveKind},
    Config, Error, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher,
};

use crate::level::Level;

#[derive(Clone)]
pub struct DataWatcher {
    baseline: u32,
    input: PathBuf,
    output: Option<PathBuf>,
    deaths: HashMap<PathBuf, u32>,
    previous: Option<u32>,
    interval: u64,
}

impl DataWatcher {
    pub fn new(baseline: u32, interval: u64, input: PathBuf, output: Option<PathBuf>) -> Self {
        Self {
            baseline,
            interval,
            input,
            output,
            deaths: HashMap::new(),
            previous: None,
        }
    }

    fn compute_deaths_file(file: File) -> anyhow::Result<u32> {
        let level: Level = serde_json::from_reader(file)?;
        Ok(level.total_deaths())
    }

    fn compute_deaths_paths(&mut self, paths: Vec<PathBuf>) -> u32 {
        paths
            .into_iter()
            .filter(|path| path.extension().map_or(false, |ext| ext == "json"))
            .filter_map(|path| {
                File::open(&path).ok().and_then(|file| {
                    Self::compute_deaths_file(file)
                        .ok()
                        .map(|deaths| (path, deaths))
                })
            })
            .for_each(|(path, deaths)| {
                self.deaths.insert(path, deaths);
            });

        self.deaths.values().sum::<u32>() + self.baseline
    }

    fn compute_deaths_dir(&mut self, directory: ReadDir) -> u32 {
        self.compute_deaths_paths(
            directory
                .filter_map(Result::ok)
                .map(|entry| entry.path().canonicalize())
                .filter_map(Result::ok)
                .collect(),
        )
    }

    fn write_deaths<P: AsRef<Path>>(&self, deaths: u32, output: P) -> anyhow::Result<()> {
        let mut file = File::create(output)?;
        write!(file, "{}", deaths)?;
        Ok(())
    }

    fn update_deaths(&mut self, deaths: u32) -> anyhow::Result<()> {
        match &self.output {
            Some(output) => self.write_deaths(deaths, output)?,
            None => println!("{}", deaths),
        };

        self.previous = Some(deaths);
        Ok(())
    }

    fn receive(&mut self, rx: &Receiver<Result<Event, Error>>) {
        match rx.recv() {
            Ok(Ok(event)) => {
                println!("{:?}", event);

                if matches!(
                    event.kind,
                    EventKind::Create(CreateKind::File)
                        | EventKind::Modify(ModifyKind::Data(DataChange::Content))
                        | EventKind::Remove(RemoveKind::File)
                ) {
                    let deaths = self.compute_deaths_paths(event.paths);

                    match self.update_deaths(deaths) {
                        Ok(_) => (),
                        Err(e) => eprintln!("Failed to update deaths: {:?}", e),
                    }
                }
            }
            Ok(Err(e)) => eprintln!("Event error: {:?}", e),
            Err(e) => eprintln!("Receiver error: {:?}", e),
        }
    }

    pub fn watch(&mut self) -> anyhow::Result<()> {
        let deaths = self.compute_deaths_dir(fs::read_dir(&self.input)?);
        self.update_deaths(deaths)?;

        let (tx, rx) = channel();
        let mut watcher = RecommendedWatcher::new(
            tx,
            Config::default().with_poll_interval(Duration::from_millis(self.interval)),
        )?;

        watcher.watch(&self.input, RecursiveMode::NonRecursive)?;

        let rx_arc = Arc::new(Mutex::new(rx));
        let self_arc = Arc::new(Mutex::new(self.clone()));

        thread::spawn(move || loop {
            let rx_lock = rx_arc.lock().unwrap();
            let mut self_lock = self_arc.lock().unwrap();
            self_lock.receive(&rx_lock);
        });

        loop {
            std::thread::park();
        }
    }
}
