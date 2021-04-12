use tracing::instrument;

use super::patterns::{CLAIM_BAD, CLAIM_GOOD};
use std::str::FromStr;

#[derive(Debug, Clone, PartialEq)]
pub enum ClaimEgs {
    Success {
        username: String,
        amount: i32,
        total: i32,
    },
    Failure {
        username: String,
        minutes: Option<u64>,
        seconds: Option<u64>,
        total: i32,
    },
}

#[derive(Debug, thiserror::Error)]
pub enum ClaimEgsParserError {
    #[error("Message is not a valid claim egs message")]
    InvalidMessage,

    #[error("Missing username in message")]
    MissingUsername,

    #[error("Missing amount in message")]
    MissingAmount,

    #[error("Missing total in message")]
    MissingTotal,

    #[error("Could not parse int: {0}")]
    ParseIntError(#[from] std::num::ParseIntError),
}

impl ClaimEgs {
    #[instrument]
    fn parse_success(s: &str) -> Result<Self, ClaimEgsParserError> {
        let captures = CLAIM_GOOD
            .captures(s)
            .ok_or(ClaimEgsParserError::InvalidMessage)?;

        let username = captures
            .name("username")
            .ok_or(ClaimEgsParserError::MissingUsername)?
            .as_str()
            .to_string();

        let amount = captures
            .name("amount")
            .ok_or(ClaimEgsParserError::MissingAmount)?
            .as_str()
            .parse()
            .map_err(|err| ClaimEgsParserError::ParseIntError(err))?;

        let total = captures
            .name("total")
            .ok_or(ClaimEgsParserError::MissingTotal)?
            .as_str()
            .parse()
            .map_err(|err| ClaimEgsParserError::ParseIntError(err))?;

        Ok(Self::Success {
            username,
            amount,
            total,
        })
    }

    #[instrument]
    fn parse_failure(s: &str) -> Result<Self, ClaimEgsParserError> {
        let captures = CLAIM_BAD
            .captures(s)
            .ok_or(ClaimEgsParserError::InvalidMessage)?;

        let username = captures
            .name("username")
            .ok_or(ClaimEgsParserError::MissingUsername)?
            .as_str()
            .to_string();

        let minutes = match captures.name("minutes") {
            Some(m) => Some(
                m.as_str()
                    .parse()
                    .map_err(|err| ClaimEgsParserError::ParseIntError(err))?,
            ),
            None => None,
        };

        let seconds = match captures.name("seconds") {
            Some(m) => Some(
                m.as_str()
                    .parse()
                    .map_err(|err| ClaimEgsParserError::ParseIntError(err))?,
            ),
            None => None,
        };

        let total = captures
            .name("total")
            .ok_or(ClaimEgsParserError::MissingTotal)?
            .as_str()
            .parse()
            .map_err(|err| ClaimEgsParserError::ParseIntError(err))?;

        Ok(Self::Failure {
            username,
            minutes,
            seconds,
            total,
        })
    }
}

impl FromStr for ClaimEgs {
    type Err = ClaimEgsParserError;

    #[instrument]
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if CLAIM_GOOD.is_match(s) {
            ClaimEgs::parse_success(s)
        } else if CLAIM_BAD.is_match(s) {
            ClaimEgs::parse_failure(s)
        } else {
            Err(ClaimEgsParserError::InvalidMessage)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_failure() {
        let text =
            "@chronophylos nam1Sadeg no eg. come back in 10 minutes, 56 seconds Total egs: 60";
        let claim_egs = text.parse::<ClaimEgs>().unwrap();

        assert_eq!(
            claim_egs,
            ClaimEgs::Failure {
                username: "chronophylos".to_string(),
                minutes: Some(10),
                seconds: Some(56),
                total: 60
            }
        );
    }

    #[test]
    fn test_success() {
        let text = "@chronophylos | is this a YOLK? nam1Okayeg | +1 egs | Total egs: 92 🥚 ";
        let claim_egs = text.parse::<ClaimEgs>().unwrap();

        assert_eq!(
            claim_egs,
            ClaimEgs::Success {
                username: "chronophylos".to_string(),
                amount: 1,
                total: 92
            }
        );
    }

    #[test]
    fn test_success2() {
        let text =
            "@chronophylos | hobos cna\'t affor egs :( nam1Hobo | +0  egs | Total egs: 152 🥚";
        let claim_egs = text.parse::<ClaimEgs>().unwrap();

        assert_eq!(
            claim_egs,
            ClaimEgs::Success {
                username: "chronophylos".to_string(),
                amount: 0,
                total: 152
            }
        );
    }
}
