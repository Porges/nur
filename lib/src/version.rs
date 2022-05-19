use std::num::ParseIntError;

#[derive(PartialOrd, Ord, PartialEq, Eq, Debug)]
pub struct Version {
    pub major: u32,
    pub minor: u32,
}

impl serde::Serialize for Version {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let as_str = format!("{}.{}", self.major, self.minor);
        serializer.serialize_str(&as_str)
    }
}

#[derive(thiserror::Error, Debug)]
pub enum ParseVersionError {
    #[error("missing '.' in version")]
    MissingDot,

    #[error("invalid number in version")]
    InvalidNumber(#[from] ParseIntError),
}

impl std::str::FromStr for Version {
    type Err = ParseVersionError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if let Some((major_s, minor_s)) = s.split_once('.') {
            let major = str::parse(major_s)?;
            let minor = str::parse(minor_s)?;
            Ok(Version { major, minor })
        } else {
            Err(ParseVersionError::MissingDot)
        }
    }
}

impl<'de> serde::Deserialize<'de> for Version {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct V;

        use serde::de::Error;
        impl<'de> serde::de::Visitor<'de> for V {
            type Value = Version;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("version number")
            }

            fn visit_str<E: Error>(self, string: &str) -> Result<Self::Value, E> {
                str::parse(string).map_err(Error::custom)
            }
        }

        deserializer.deserialize_str(V)
    }
}
