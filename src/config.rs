use anyhow::Result;
use ron::de::from_reader;
use serde::Deserialize;
use std::{fs::File, path::Path};

use crate::{leavesbot, SecretToken};

#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    pub username: String,
    pub token: SecretToken,
    pub cookiebot_channel: String,
    pub egbot_channel: String,
    pub cookiebot_disabled: bool,
    pub egbot_disabled: bool,
    pub leavesbot: leavesbot::Config,
}

impl Config {
    pub fn from_path<P>(path: P) -> Result<Self>
    where
        P: AsRef<Path>,
    {
        Ok(from_reader(File::open(path)?)?)
    }
}
