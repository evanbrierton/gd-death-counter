use serde::Deserialize;
use std::collections::HashMap;

#[derive(Deserialize, Debug)]
pub struct Level {
    deaths: HashMap<String, u32>,
    runs: HashMap<String, u32>,
}

impl Level {
    pub fn total_deaths(&self) -> u32 {
        self.deaths.values().sum::<u32>() + self.runs.values().sum::<u32>()
    }
}
