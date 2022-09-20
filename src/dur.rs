use crate::error::{Error, Result};
use regex::Regex;
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Debug, Clone, Copy)]
pub struct Duration(pub u16, pub u16);

impl Duration {
    pub fn new_checked(num: u16, den: u16) -> Result<Self> {
        if den == 0 {
            Err(Error::InvalidOp("Duration with 0 denominator".into()))
        } else if num == 0 {
            Ok(Self(0, 1))
        } else if den == 1 || num == 1 {
            Ok(Self(num, den))
        } else {
            for i in (1..=u16::min(num, den)).rev() {
                if num % i == 0 && den % i == 0 {
                    return Ok(Self(num / i, den / i));
                }
            }
            Err(Error::InvalidOp("Cannot find GCD".into()))
        }
    }

    pub fn new(num: u16, den: u16) -> Self {
        if den == 0 {
            panic!();
        } else if num == 0 {
            Self(0, 1)
        } else if den == 1 || num == 1 {
            Self(num, den)
        } else {
            for i in (1..=u16::min(num, den)).rev() {
                if num % i == 0 && den % i == 0 {
                    return Self(num / i, den / i);
                }
            }
            panic!("{} {}", num, den);
        }
    }

    fn new_pow2(mut num: u16, mut den: u16) -> Self {
        while den > 1 && (num & 1) == 0 {
            num /= 2;
            den /= 2;
        }
        Self(num, den)
    }

    pub fn tuplet(&self, tuplet: u16) -> Self {
        Self::new(self.0 * 2, self.1 * tuplet)
    }

    pub fn dotted(&self) -> Self {
        Self::new(self.0 * 3, self.1 * 2)
    }

    pub fn whole(count: u16) -> Self {
        Self(count, 1)
    }

    pub fn half(count: u16) -> Self {
        Self::new_pow2(count, 2)
    }

    pub fn quarter(count: u16) -> Self {
        Self::new_pow2(count, 4)
    }

    pub fn eighth(count: u16) -> Self {
        Self::new_pow2(count, 8)
    }

    pub fn sixteenth(count: u16) -> Self {
        Self::new_pow2(count, 16)
    }

    pub fn thirtysecond(count: u16) -> Self {
        Self::new_pow2(count, 32)
    }

    pub fn sixtyfourth(count: u16) -> Self {
        Self::new_pow2(count, 64)
    }

    pub fn zero() -> Self {
        Self(0, 1)
    }

    pub fn add_basic(self, rhs: Self) -> Self {
        Self(self.0 * rhs.1 + rhs.0 * self.1, self.1 * rhs.1)
    }

    pub fn dur_icon(&self) -> &'static str {
        match self {
            Self(1, 1) => " 1 ",
            Self(1, 2) => " 2 ",
            Self(1, 4) => " 4 ",
            Self(1, 8) => " 8 ",
            Self(1, 16) => "16 ",
            Self(1, 32) => "32 ",
            Self(3, 2) => " 1•",
            Self(3, 4) => " 2•",
            Self(3, 8) => " 4•",
            Self(3, 16) => " 8•",
            Self(3, 32) => "16•",
            Self(1, 3) => " 2⅓",
            Self(1, 6) => " 4⅓",
            Self(1, 12) => " 8⅓",
            Self(1, 24) => "16⅓",
            Self(1, 48) => "32⅓",
            Self(1, 96) => "64⅓",
            _ => " ? ",
        }
    }
}

impl std::str::FromStr for Duration {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        lazy_static::lazy_static! {
            static ref RE: Regex = Regex::new(r"^(?:(\d+)/|)(\d+)(\.|)(?::(\d+)|)$").unwrap();
        }
        fn parse_match<T: std::str::FromStr>(opt: Option<regex::Match>) -> Result<Option<T>> {
            if let Some(v) = opt {
                match v.as_str().parse::<T>() {
                    Ok(v) => Ok(Some(v)),
                    _ => Err(Error::ParseError(format!(
                        "Unable to parse '{opt:?}' as value"
                    ))),
                }
            } else {
                Ok(None)
            }
        }
        if let Some(caps) = RE.captures(s) {
            let num = parse_match(caps.get(1))?;
            let base = parse_match(caps.get(2))?;
            let tuplet = parse_match(caps.get(4))?;
            let dotted = caps.get(3).unwrap().range().len() > 0;

            if let Some(base) = base {
                let mut d = Duration::new_checked(1, base)?;
                if dotted {
                    d = d.dotted();
                }
                if num.is_some() {
                    d = d * num.unwrap();
                }
                if tuplet.is_some() {
                    d = (d / tuplet.unwrap()) * 2;
                }
                return Ok(d);
            }
        }
        Err(Error::ParseError(format!(
            "Unable to parse '{s}' as Duration"
        )))
    }
}

impl std::ops::Add<Self> for Duration {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self::new(self.0 * rhs.1 + rhs.0 * self.1, self.1 * rhs.1)
    }
}

impl std::ops::Sub<Self> for Duration {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        Self::new(
            (self.0 * rhs.1).saturating_sub(rhs.0 * self.1),
            self.1 * rhs.1,
        )
    }
}

impl std::ops::Mul<u16> for Duration {
    type Output = Self;

    fn mul(self, rhs: u16) -> Self::Output {
        Self::new(self.0 * rhs, self.1)
    }
}

impl std::ops::Div<u16> for Duration {
    type Output = Self;

    fn div(self, rhs: u16) -> Self::Output {
        Self::new(self.0, self.1 * rhs)
    }
}

impl PartialOrd for Duration {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        if self.1 == other.1 {
            self.0.partial_cmp(&other.0)
        } else {
            let a = self.0 * other.1;
            let b = other.0 * self.1;
            u16::partial_cmp(&a, &b)
        }
    }
}

impl Ord for Duration {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        if self.1 == other.1 {
            self.0.cmp(&other.0)
        } else {
            let a = self.0 * other.1;
            let b = other.0 * self.1;
            u16::cmp(&a, &b)
        }
    }
}

impl PartialEq for Duration {
    fn eq(&self, other: &Self) -> bool {
        if self.1 == other.1 {
            self.0 == other.0
        } else {
            self.0 * other.1 == other.0 * self.1
        }
    }
}

impl Eq for Duration {}
