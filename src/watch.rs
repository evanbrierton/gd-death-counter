use std::{
    collections::HashMap,
    fs::{self, File, ReadDir},
    io::Write,
    path::{self, Path, PathBuf},
    sync::{
        mpsc::{channel, Receiver},
        Arc, Mutex,
    },
    thread,
    time::Duration,
};

use notify::{
    event::{ModifyKind, RenameMode},
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

    fn get_total_deaths(&self) -> u32 {
        self.deaths.values().sum::<u32>() + self.baseline
    }

    fn compute_deaths_file(file: File) -> anyhow::Result<u32> {
        let level: Level = serde_json::from_reader(file)?;
        Ok(level.total_deaths())
    }

    fn compute_deaths_paths(&mut self, paths: Vec<PathBuf>) {
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
    }

    fn compute_deaths_dir(&mut self, directory: ReadDir) {
        self.compute_deaths_paths(
            directory
                .filter_map(|entry| entry.ok().map(|entry| entry.path()))
                .map(path::absolute)
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
        if let Some(previous) = self.previous {
            if deaths == previous {
                return Ok(());
            }
        }

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
                match event.kind {
                    EventKind::Modify(ModifyKind::Name(RenameMode::From))
                    | EventKind::Remove(_) => event.paths.iter().for_each(|path| {
                        self.deaths.remove(path);
                    }),
                    EventKind::Create(_) | EventKind::Modify(_) => {
                        self.compute_deaths_paths(event.paths)
                    }
                    _ => {}
                }

                if matches!(
                    event.kind,
                    EventKind::Remove(_) | EventKind::Create(_) | EventKind::Modify(_)
                ) {
                    match self.update_deaths(self.get_total_deaths()) {
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
        self.compute_deaths_dir(fs::read_dir(&self.input)?);
        self.update_deaths(self.get_total_deaths())?;

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
