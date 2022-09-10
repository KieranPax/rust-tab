use crate::song::{Beat, Note};
use std::fmt;

pub enum Buffer {
    Empty,
    Note(Note),
    Beat(Beat),
    Beats(Vec<Beat>),
}

impl fmt::Debug for Buffer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => write!(f, "Empty"),
            Self::Note(_) => write!(f, "Note"),
            Self::Beat(_) => write!(f, "Beat"),
            Self::Beats(_) => write!(f, "MultiBeat"),
        }
    }
}
