//! Profile name validation and type.

use std::fmt;

/// A validated profile name.
///
/// Profile names must be:
/// - 1-64 characters
/// - Lowercase alphanumeric with hyphens
/// - No leading or trailing hyphens
/// - No consecutive hyphens
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize)]
pub struct ProfileName(String);

impl ProfileName {
    pub const MAX_LENGTH: usize = 64;

    /// Create a new profile name, validating the input.
    ///
    /// # Errors
    ///
    /// Returns an error if the name is invalid.
    pub fn new(name: &str) -> Result<Self, InvalidProfileName> {
        Self::validate(name)?;
        Ok(Self(name.to_lowercase()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    fn validate(name: &str) -> Result<(), InvalidProfileName> {
        if name.is_empty() {
            return Err(InvalidProfileName::Empty);
        }

        if name.len() > Self::MAX_LENGTH {
            return Err(InvalidProfileName::TooLong(name.len()));
        }

        if name.starts_with('-') || name.ends_with('-') {
            return Err(InvalidProfileName::LeadingOrTrailingHyphen);
        }

        if name.contains("--") {
            return Err(InvalidProfileName::ConsecutiveHyphens);
        }

        for c in name.chars() {
            if !c.is_ascii_alphanumeric() && c != '-' {
                return Err(InvalidProfileName::InvalidCharacter(c));
            }
        }

        Ok(())
    }
}

impl fmt::Display for ProfileName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl TryFrom<&str> for ProfileName {
    type Error = InvalidProfileName;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl TryFrom<String> for ProfileName {
    type Error = InvalidProfileName;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(&value)
    }
}

impl AsRef<str> for ProfileName {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

/// Errors that can occur when validating a profile name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InvalidProfileName {
    /// Profile name is empty.
    Empty,
    /// Profile name exceeds maximum length.
    TooLong(usize),
    /// Profile name starts or ends with a hyphen.
    LeadingOrTrailingHyphen,
    /// Profile name contains consecutive hyphens.
    ConsecutiveHyphens,
    /// Profile name contains an invalid character.
    InvalidCharacter(char),
}

impl fmt::Display for InvalidProfileName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => write!(f, "profile name cannot be empty"),
            Self::TooLong(len) => write!(
                f,
                "profile name too long ({len} chars, max {})",
                ProfileName::MAX_LENGTH
            ),
            Self::LeadingOrTrailingHyphen => {
                write!(f, "profile name cannot start or end with a hyphen")
            }
            Self::ConsecutiveHyphens => {
                write!(f, "profile name cannot contain consecutive hyphens")
            }
            Self::InvalidCharacter(c) => {
                write!(
                    f,
                    "invalid character '{c}': only lowercase alphanumeric and hyphens allowed"
                )
            }
        }
    }
}

impl std::error::Error for InvalidProfileName {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_profile_names() {
        assert!(ProfileName::new("default").is_ok());
        assert!(ProfileName::new("my-profile").is_ok());
        assert!(ProfileName::new("profile123").is_ok());
        assert!(ProfileName::new("a").is_ok());
        assert!(ProfileName::new("test-profile-name").is_ok());
    }

    #[test]
    fn normalizes_to_lowercase() {
        let name = ProfileName::new("MyProfile").unwrap();
        assert_eq!(name.as_str(), "myprofile");
    }

    #[test]
    fn rejects_empty_name() {
        assert_eq!(ProfileName::new(""), Err(InvalidProfileName::Empty));
    }

    #[test]
    fn rejects_too_long_name() {
        let long_name = "a".repeat(65);
        assert!(matches!(
            ProfileName::new(&long_name),
            Err(InvalidProfileName::TooLong(65))
        ));
    }

    #[test]
    fn rejects_leading_hyphen() {
        assert_eq!(
            ProfileName::new("-profile"),
            Err(InvalidProfileName::LeadingOrTrailingHyphen)
        );
    }

    #[test]
    fn rejects_trailing_hyphen() {
        assert_eq!(
            ProfileName::new("profile-"),
            Err(InvalidProfileName::LeadingOrTrailingHyphen)
        );
    }

    #[test]
    fn rejects_consecutive_hyphens() {
        assert_eq!(
            ProfileName::new("my--profile"),
            Err(InvalidProfileName::ConsecutiveHyphens)
        );
    }

    #[test]
    fn rejects_invalid_characters() {
        assert!(matches!(
            ProfileName::new("my_profile"),
            Err(InvalidProfileName::InvalidCharacter('_'))
        ));
        assert!(matches!(
            ProfileName::new("my profile"),
            Err(InvalidProfileName::InvalidCharacter(' '))
        ));
        assert!(matches!(
            ProfileName::new("my.profile"),
            Err(InvalidProfileName::InvalidCharacter('.'))
        ));
    }

    #[test]
    fn try_from_str() {
        let name: Result<ProfileName, _> = "valid-name".try_into();
        assert!(name.is_ok());
    }

    #[test]
    fn try_from_string() {
        let name: Result<ProfileName, _> = String::from("valid-name").try_into();
        assert!(name.is_ok());
    }
}
