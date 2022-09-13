use crate::error::{Error, Result, SResult};
use regex::Regex;
use serde::{
    de::{self, Visitor},
    ser::SerializeTuple,
    Deserialize, Serialize,
};
use std::fmt;

type Fraction = fraction::GenericFraction<u8>;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd)]
pub struct Duration(Fraction);

impl fmt::Debug for Duration {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("Duration").field(&self.tuple()).finish()
    }
}

fn parse_match<T: std::str::FromStr>(opt: Option<regex::Match>) -> Option<T> {
    if let Some(v) = opt {
        v.as_str().parse::<T>().ok()
    } else {
        None
    }
}

impl Duration {
    pub fn new(a: u8, b: u8) -> Self {
        Self(Fraction::new(a, b))
    }

    pub fn num(&self) -> u8 {
        *self.0.numer().unwrap()
    }

    pub fn dem(&self) -> u8 {
        *self.0.denom().unwrap()
    }

    pub fn tuple(&self) -> (u8, u8) {
        (self.num(), self.dem())
    }

    pub fn dotted(&self) -> Self {
        Self::new(self.num() * 3, self.dem() * 2)
    }

    pub fn dur_icon(&self) -> &'static str {
        match self.tuple() {
            (1, 1) => " 1 ",
            (1, 2) => " 2 ",
            (1, 4) => " 4 ",
            (1, 8) => " 8 ",
            (1, 16) => "16 ",
            (1, 32) => "32 ",
            (3, 2) => " 1•",
            (3, 4) => " 2•",
            (3, 8) => " 4•",
            (3, 16) => " 8•",
            (3, 32) => "16•",
            (1, 3) => " 2⅓",
            (1, 6) => " 4⅓",
            (1, 12) => " 8⅓",
            (1, 24) => "16⅓",
            (1, 48) => "32⅓",
            (1, 96) => "64⅓",
            _ => " ? ",
        }
    }

    pub fn parse(s: &str) -> Result<Self> {
        lazy_static::lazy_static! {
            static ref RE: Regex = Regex::new(r"^(?:(\d+)/|)(\d+)(\.|)(?::(\d+)|)$").unwrap();
        }
        if let Some(caps) = RE.captures(s) {
            let num = parse_match(caps.get(1));
            let base = parse_match(caps.get(2));
            let tuplet = parse_match(caps.get(4));
            let dotted = caps.get(3).unwrap().range().len() > 0;

            if let Some(base) = base {
                let mut d = Duration::new(1, base);
                if dotted {
                    d = d.dotted();
                }
                if num.is_some() {
                    d = d * num.unwrap();
                }
                if tuplet.is_some() {
                    d = (d / tuplet.unwrap()) * 2;
                }
                Ok(d)
            } else {
                Err(Error::InvalidOp(format!(
                    "Unable to parse '{s}' as Duration"
                )))
            }
        } else {
            Err(Error::InvalidOp(format!(
                "Unable to parse '{s}' as Duration"
            )))
        }
    }
}

impl Serialize for Duration {
    fn serialize<S>(&self, serializer: S) -> SResult<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut seq = serializer.serialize_tuple(2)?;
        seq.serialize_element(&self.num())?;
        seq.serialize_element(&self.dem())?;
        seq.end()
    }
}

struct DurationVisitor;

impl<'de> Visitor<'de> for DurationVisitor {
    type Value = (u8, u8);

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("two integers between 0 and 255")
    }

    fn visit_seq<A>(self, mut seq: A) -> SResult<Self::Value, A::Error>
    where
        A: de::SeqAccess<'de>,
    {
        let a = seq.next_element()?.unwrap();
        let b = seq.next_element()?.unwrap();
        Ok((a, b))
    }
}

impl<'de> Deserialize<'de> for Duration {
    fn deserialize<D>(deserializer: D) -> SResult<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let t = deserializer.deserialize_tuple(2, DurationVisitor)?;
        Ok(Duration::new(t.0, t.1))
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

impl std::ops::Mul<u8> for Duration {
    type Output = Self;

    fn mul(self, rhs: u8) -> Self::Output {
        Duration(self.0 * Fraction::new(rhs, 1))
    }
}

impl std::ops::Div<u8> for Duration {
    type Output = Self;

    fn div(self, rhs: u8) -> Self::Output {
        Duration(self.0 * Fraction::new(1, rhs))
    }
}
