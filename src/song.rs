use crate::{
    dur::Duration,
    error::{Error, Result},
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Note {
    Fret(u16),
    X,
}

impl Note {
    pub fn parse(s: &str) -> Result<Self> {
        if s == "x" {
            Ok(Self::X)
        } else if let Ok(fret) = s.parse() {
            Ok(Self::Fret(fret))
        } else {
            Err(Error::InvalidOp(format!("Cannot parse '{s}' as note")))
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Beat {
    pub dur: Duration,
    pub notes: Vec<(u16, Note)>,
}

impl Beat {
    pub fn new(dur: Duration) -> Self {
        Self {
            dur,
            notes: Vec::new(),
        }
    }

    pub fn copy_duration(&self) -> Self {
        Self::new(self.dur)
    }

    pub fn copy_note(&self, string: u16) -> Option<Note> {
        for i in self.notes.iter() {
            if i.0 == string {
                return Some(i.1.to_owned());
            }
        }
        None
    }

    pub fn get_note(&self, string: u16) -> Option<&Note> {
        for i in self.notes.iter() {
            if i.0 == string {
                return Some(&i.1);
            }
        }
        None
    }

    pub fn set_note(&mut self, string: u16, note: Note) {
        for i in self.notes.iter_mut() {
            if i.0 == string {
                i.1 = note;
                return;
            }
        }
        self.notes.push((string, note));
    }

    pub fn del_note(&mut self, string: u16) {
        for i in 0..self.notes.len() {
            if self.notes[i].0 == string {
                self.notes.swap_remove(i);
                return;
            }
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct Track {
    pub string_count: u16,
    pub beats: Vec<Beat>,
    #[serde(skip)]
    pub measure_i: Vec<bool>,
}

impl Track {
    pub fn new() -> Self {
        Self {
            string_count: 6,
            beats: vec![Beat::new(Duration::new(1, 1))],
            measure_i: Vec::new(),
        }
    }

    pub fn update_measures(&mut self) {
        self.measure_i.clear();
        self.measure_i.reserve(self.beats.len());
        let mut total = Duration::new(1, 1);
        let mlen = Duration::new(1, 1);
        for beat in self.beats.iter() {
            if total == mlen {
                total = Duration::new(0, 1);
                self.measure_i.push(true);
            } else if total > mlen {
                total = total - mlen;
                self.measure_i.push(false);
            } else {
                self.measure_i.push(false);
            }
            total = total + beat.dur;
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct Song {
    pub tracks: Vec<Track>,
}

impl Song {
    pub fn new() -> Self {
        Self {
            tracks: vec![Track::new()],
        }
    }
}
