use crate::error::Error;
use serde::{
    de::{self, Visitor},
    ser::SerializeTuple,
    Deserialize, Serialize,
};
use std::fmt;

type Fraction = fraction::GenericFraction<u8>;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd)]
pub struct Duration(Fraction);

impl Duration {
    pub fn new(a: u8, b: u8) -> Self {
        Self(Fraction::new(a, b))
    }

    pub fn tuple(&self) -> (u8, u8) {
        (*self.0.numer().unwrap(), *self.0.denom().unwrap())
    }
}

impl Serialize for Duration {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut seq = serializer.serialize_tuple(2)?;
        seq.serialize_element(&self.0.numer())?;
        seq.serialize_element(&self.0.denom())?;
        seq.end()
    }
}

struct DurationVisitor;

impl<'de> Visitor<'de> for DurationVisitor {
    type Value = (u8, u8);

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("two integers between 0 and 255")
    }

    fn visit_seq<A>(self, mut seq: A) -> std::result::Result<Self::Value, A::Error>
    where
        A: de::SeqAccess<'de>,
    {
        let a = seq.next_element()?.unwrap();
        let b = seq.next_element()?.unwrap();
        Ok((a, b))
    }
}

impl<'de> Deserialize<'de> for Duration {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let t = deserializer.deserialize_tuple(2, DurationVisitor)?;
        Ok(Duration::new(t.0, t.1))
    }
}

impl fmt::Display for Duration {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.tuple() {
            (1, 1) => f.write_str(" 1 "),
            (1, 2) => f.write_str(" 2 "),
            (1, 4) => f.write_str(" 4 "),
            (1, 8) => f.write_str(" 8 "),
            (1, 16) => f.write_str("16 "),
            (1, 32) => f.write_str("32 "),
            (3, 2) => f.write_str(" 1•"),
            (3, 4) => f.write_str(" 2•"),
            (3, 8) => f.write_str(" 4•"),
            (3, 16) => f.write_str(" 8•"),
            (3, 32) => f.write_str("16•"),
            (1, 3) => f.write_str(" 2⅓"),
            (1, 6) => f.write_str(" 4⅓"),
            (1, 12) => f.write_str(" 8⅓"),
            (1, 24) => f.write_str("16⅓"),
            (1, 48) => f.write_str("32⅓"),
            (1, 96) => f.write_str("64⅓"),
            _ => f.write_str(" ? "),
        }
    }
}

impl std::str::FromStr for Duration {
    type Err = Error;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s {
            "1" => Ok(Self::new(1, 1)),
            "2" => Ok(Self::new(1, 2)),
            "4" => Ok(Self::new(1, 4)),
            "8" => Ok(Self::new(1, 8)),
            "16" => Ok(Self::new(1, 16)),
            "32" => Ok(Self::new(1, 32)),
            _ => {
                if let Some((a, b)) = s.split_once('/') {
                    let (a, b) = (a.parse(), b.parse());
                    if a.is_err() || b.is_err() {
                        Err(Error::InvalidOp(format!("Cannot parse '{s}' as Duration")))
                    } else {
                        Ok(Self::new(a.unwrap(), b.unwrap()))
                    }
                } else {
                    Err(Error::InvalidOp(format!("Cannot parse '{s}' as Duration")))
                }
            }
        }
    }
}

impl std::ops::Add for Duration {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Duration(self.0 + rhs.0)
    }
}

impl std::ops::Sub for Duration {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        Duration(self.0 - rhs.0)
    }
}
