use std::collections::HashMap;
use std::fs::File;
use std::io::prelude::*;
use std::sync::{Arc};
use std::path::Path;

use irc::client::data::Config as IrcConfig;
use lazy_static::lazy_static;
use parking_lot::RwLock;
use regex::Regex;
use serde_derive::Deserialize;

use crate::Error;


#[derive(Clone, Default, Deserialize)]
pub struct Config {
    #[serde(skip)]
    revision: u32,

    pub discord: DiscordConfig,
    #[serde(default)]
    pub irc: IrcConfig,

    #[serde(default)]
    #[serde(with = "bots")]
    pub bots: HashMap<String, Regex>,
}

#[derive(Clone, Default, Deserialize)]
pub struct DiscordConfig {
    pub bot_token: String,
}

impl Config {
    pub fn from_path(path: impl AsRef<Path>) -> Result<Self, Error> {
        let mut f = File::open(path.as_ref())?;
        let mut contents = Vec::new();
        f.read_to_end(&mut contents)?;
        toml::from_slice(&contents).map_err(|e| Error::configuration(e))
    }

    pub fn revision(&self) -> u32 { self.revision }
}

lazy_static! {
    static ref CONFIG: RwLock<Arc<Config>> = Default::default();
}

pub fn update(mut config: Config) {
    let mut lock = CONFIG.write();
    config.revision = lock.revision + 1;
    *lock = Arc::new(config);
}

pub fn fetch_config() -> Arc<Config> {
    Arc::clone(&CONFIG.read())
}

mod bots {
    use serde::de;

    use super::*;

    #[inline]
    pub(super) fn deserialize<'de, D>(de: D) -> Result<HashMap<String, Regex>, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        let s: HashMap<String, &str> = de::Deserialize::deserialize(de)?;
        let m = s.into_iter()
            .map(|(k, v)| Regex::new(v).map(|v| (k, v)))
            .collect::<Result<_, _>>();
        m.map_err(de::Error::custom)
    }
}
