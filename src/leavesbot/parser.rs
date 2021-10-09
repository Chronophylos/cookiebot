use std::str::FromStr;

use tracing::instrument;

use super::patterns::{CLAIM_BAD, CLAIM_GOOD};

#[derive(Debug, thiserror::Error)]
pub enum ClaimResponseParserError {
    #[error("Message is not a valid claim response")]
    InvalidMessage,

    #[error("Expected cooldown variant but got something else")]
    ExpectedBad,

    #[error("Expected success variant but got something else")]
    ExpectedSuccess,

    #[error("Missing username in message")]
    MissingUsername,

    #[error("Missing amount in message")]
    MissingAmount,

    #[error("Missing total in message")]
    MissingTotal,

    #[error("Could not parse int: {0}")]
    ParseIntError(#[from] std::num::ParseIntError),
}

#[derive(Debug, Clone, PartialEq)]
pub enum ClaimResponse {
    Success {
        username: String,
        amount: i32,
        total: i32,
    },
    Cooldown {
        username: String,
        minutes: Option<u64>,
        seconds: Option<u64>,
        total: i32,
    },
}

impl ClaimResponse {
    #[instrument]
    fn parse_success(s: &str) -> Result<Self, ClaimResponseParserError> {
        let captures = CLAIM_GOOD
            .captures(s)
            .ok_or(ClaimResponseParserError::ExpectedSuccess)?;

        let username = captures
            .name("username")
            .ok_or(ClaimResponseParserError::MissingUsername)?
            .as_str()
            .to_string();

        let amount = captures
            .name("amount")
            .ok_or(ClaimResponseParserError::MissingAmount)?
            .as_str()
            .parse()
            .map_err(ClaimResponseParserError::ParseIntError)?;

        let total = captures
            .name("total")
            .ok_or(ClaimResponseParserError::MissingAmount)?
            .as_str()
            .parse()
            .map_err(ClaimResponseParserError::ParseIntError)?;

        Ok(Self::Success {
            username,
            amount,
            total,
        })
    }

    #[instrument]
    fn parse_cooldown(s: &str) -> Result<Self, ClaimResponseParserError> {
        let captures = CLAIM_BAD
            .captures(s)
            .ok_or(ClaimResponseParserError::ExpectedBad)?;

        let username = captures
            .name("username")
            .ok_or(ClaimResponseParserError::MissingUsername)?
            .as_str()
            .to_string();

        let minutes = captures
            .name("minutes")
            .map(|m| {
                m.as_str()
                    .parse()
                    .map_err(ClaimResponseParserError::ParseIntError)
            })
            .transpose()?;

        let seconds = captures
            .name("seconds")
            .map(|m| {
                m.as_str()
                    .parse()
                    .map_err(ClaimResponseParserError::ParseIntError)
            })
            .transpose()?;

        let total = captures
            .name("total")
            .ok_or(ClaimResponseParserError::MissingTotal)?
            .as_str()
            .parse()
            .map_err(ClaimResponseParserError::ParseIntError)?;

        Ok(Self::Cooldown {
            username,
            minutes,
            seconds,
            total,
        })
    }
}

impl FromStr for ClaimResponse {
    type Err = ClaimResponseParserError;

    #[instrument]
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if CLAIM_GOOD.is_match(s) {
            ClaimResponse::parse_success(s)
        } else if CLAIM_BAD.is_match(s) {
            ClaimResponse::parse_cooldown(s)
        } else {
            Err(ClaimResponseParserError::InvalidMessage)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_failure() {
        let text =
            "🍃 @chronophylos > FeelsBadMan You need to wait 54:04 minutes until you can get more leaves | You've got 34 leaves 🍃 ";
        let claim = text.parse::<ClaimResponse>().unwrap();

        assert_eq!(
            claim,
            ClaimResponse::Cooldown {
                username: "chronophylos".to_string(),
                minutes: Some(54),
                seconds: Some(04),
                total: 34
            }
        );
    }
}
