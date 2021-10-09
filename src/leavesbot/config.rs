use serde::Deserialize;

#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    pub disabled: bool,
    pub channel: String,
}
