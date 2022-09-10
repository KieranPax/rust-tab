use crate::dur::Duration;
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize)]
pub struct Note {
    pub string: u16,
    pub fret: u32,
}

impl Note {
    pub fn new(string: u16, fret: u32) -> Self {
        Self { string, fret }
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Beat {
    pub dur: Duration,
    pub notes: Vec<Note>,
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

    pub fn get_note(&self, string: u16) -> Option<&Note> {
        for i in self.notes.iter() {
            if i.string == string {
                return Some(i);
            }
        }
        None
    }

    pub fn set_note(&mut self, string: u16, fret: u32) {
        for i in self.notes.iter_mut() {
            if i.string == string {
                i.fret = fret;
                return;
            }
        }
        self.notes.push(Note::new(string, fret))
    }

    pub fn del_note(&mut self, string: u16) {
        for i in 0..self.notes.len() {
            if self.notes[i].string == string {
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
}

impl Track {
    pub fn new() -> Self {
        Self {
            string_count: 6,
            beats: vec![Beat::new(Duration::new(1, 1))],
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct Song {
    pub tracks: Vec<Track>,
    #[serde(skip)]
    pub measure_i: Vec<usize>,
}

impl Song {
    pub fn new() -> Self {
        Self {
            tracks: vec![Track::new()],
            measure_i: Vec::new(),
        }
    }
}
