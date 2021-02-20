use serde::Deserialize;
use std::{fmt::Display, str::FromStr};
use thiserror::Error;

#[derive(Debug, Copy, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Rank {
    Default,
    Bronze,
    Silver,
    Gold,
    Platinum,
    Diamond,
    Masters,
    GrandMasters,
    Leader,
}

impl Display for Rank {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::Default => "default",
            Self::Bronze => "bronze",
            Self::Silver => "silver",
            Self::Gold => "gold",
            Self::Platinum => "platinum",
            Self::Diamond => "diamond",
            Self::Masters => "masters",
            Self::GrandMasters => "grandmasters",
            Self::Leader => "leader",
        };

        write!(f, "{}", s)
    }
}

#[derive(Debug, Error)]
pub enum ParseRankError {
    #[error("unknown rank name")]
    UnkownRankError,
}

impl FromStr for Rank {
    type Err = ParseRankError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "default" => Ok(Self::Default),
            "bronze" => Ok(Self::Bronze),
            "silver" => Ok(Self::Silver),
            "gold" => Ok(Self::Gold),
            "platinum" => Ok(Self::Platinum),
            "diamond" => Ok(Self::Diamond),
            "masters" => Ok(Self::Masters),
            "grandmasters" => Ok(Self::GrandMasters),
            "leader" => Ok(Self::Leader),
            _ => Err(Self::Err::UnkownRankError),
        }
    }
}
