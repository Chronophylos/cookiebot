use std::{fmt::Display, num::ParseIntError, str::FromStr};
use thiserror::Error;
use tracing::instrument;

use super::{
    patterns::{CLAIM_BAD, CLAIM_GOOD},
    rank::{ParseRankError, Rank},
};

#[derive(Debug, Error)]
pub enum ParsePresigeRankError {
    #[error("Missing character P")]
    MissingP,

    #[error("Missing prestige part before :")]
    MissingPrestigePart,

    #[error("Missing rank part after :")]
    MissingRankPart,

    #[error("Could not parse prestige")]
    ParsePrestigeError(#[source] ParseIntError),

    #[error("Could not parse rank")]
    ParseRankError(#[source] ParseRankError),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PrestigeRank {
    pub prestige: u32,
    pub rank: Rank,
}

impl Display for PrestigeRank {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "P{}: {}", self.prestige, self.rank)
    }
}

impl FromStr for PrestigeRank {
    type Err = ParsePresigeRankError;

    #[instrument]
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = s.strip_prefix("P").ok_or(Self::Err::MissingP)?;
        let mut split = s.split(':');

        let prestige = split
            .next()
            .ok_or(Self::Err::MissingPrestigePart)?
            .parse()
            .map_err(Self::Err::ParsePrestigeError)?;

        let rank = split
            .next()
            .ok_or(Self::Err::MissingRankPart)?
            .trim()
            .parse()
            .map_err(Self::Err::ParseRankError)?;

        Ok(PrestigeRank { prestige, rank })
    }
}

/// Result of a claim cookie command
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClaimCookieResponse {
    /// Command was successful
    Success {
        rank: PrestigeRank,
        name: String,
        amount: i32,
        total: u64,
    },

    /// Command is on cooldown
    Cooldown { rank: PrestigeRank, total: u64 },
}

#[derive(Debug, Error)]
pub enum ParseClaimCookieError {
    #[error("Regex match is missing named capture group {0}")]
    MissingCaptureGroup(&'static str),

    #[error("Could not parse prestige and rank")]
    ParsePrestigeRankError(#[from] ParsePresigeRankError),

    #[error("Could not parse int")]
    ParseIntError(#[from] ParseIntError),

    #[error("Input did not match regex")]
    InvalidInput,
}

impl FromStr for ClaimCookieResponse {
    type Err = ParseClaimCookieError;

    #[instrument]
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if let Some(captures) = CLAIM_GOOD.captures(s) {
            let rank = captures
                .name("rank")
                .ok_or(Self::Err::MissingCaptureGroup("rank"))?
                .as_str()
                .parse()?;

            let name = captures
                .name("cookie")
                .map(|m| m.as_str())
                .ok_or(Self::Err::MissingCaptureGroup("cookie"))?
                .to_string();

            let amount = captures
                .name("amount")
                .ok_or(Self::Err::MissingCaptureGroup("amount"))?
                .as_str()
                .trim_start_matches('±')
                .parse()?;

            let total = captures
                .name("total")
                .ok_or(Self::Err::MissingCaptureGroup("total"))?
                .as_str()
                .parse()?;

            Ok(Self::Success {
                rank,
                name,
                amount,
                total,
            })
        } else if let Some(captures) = CLAIM_BAD.captures(s) {
            let rank = captures
                .name("rank")
                .ok_or(Self::Err::MissingCaptureGroup("rank"))?
                .as_str()
                .parse()?;

            let total = captures
                .name("total")
                .ok_or(Self::Err::MissingCaptureGroup("total"))?
                .as_str()
                .parse()?;

            Ok(Self::Cooldown { rank, total })
        } else {
            Err(Self::Err::InvalidInput)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{ClaimCookieResponse, PrestigeRank};
    use crate::thepositivebot::rank::Rank;

    #[test]
    fn parse_claimcookie() {
        let input = "[Cookies] [P6: default] chronophylos you have already claimed a cookie and have 4957 of them! 🍪 Please wait in 2 hour intervals! ";
        let response = input.parse::<ClaimCookieResponse>().unwrap();

        assert_eq!(
            response,
            ClaimCookieResponse::Cooldown {
                rank: PrestigeRank {
                    prestige: 6,
                    rank: Rank::Default
                },
                total: 4957
            }
        )
    }
}
