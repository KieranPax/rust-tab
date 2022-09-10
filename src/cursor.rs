use crate::{
    dur::Duration,
    song::{Beat, Note, Song, Track},
};

#[derive(Clone)]
pub struct Cursor {
    pub scroll: usize,
    pub track: usize,
    pub beat: usize,
    pub string: u16,
}

impl Cursor {
    pub fn new() -> Self {
        Self {
            scroll: 0,
            track: 0,
            beat: 0,
            string: 0,
        }
    }

    pub fn track<'a>(&self, song: &'a Song) -> &'a Track {
        &song.tracks[self.track]
    }

    pub fn track_mut<'a>(&self, song: &'a mut Song) -> &'a mut Track {
        &mut song.tracks[self.track]
    }

    pub fn beats<'a>(&self, song: &'a Song) -> &'a Vec<Beat> {
        &song.tracks[self.track].beats
    }

    pub fn beats_mut<'a>(&self, song: &'a mut Song) -> &'a mut Vec<Beat> {
        &mut song.tracks[self.track].beats
    }

    pub fn beat<'a>(&self, song: &'a Song) -> &'a Beat {
        &song.tracks[self.track].beats[self.beat]
    }

    pub fn beat_mut<'a>(&self, song: &'a mut Song) -> &'a mut Beat {
        &mut song.tracks[self.track].beats[self.beat]
    }

    pub fn beat_i<'a>(&self, song: &'a Song, index: usize) -> &'a Beat {
        &song.tracks[self.track].beats[index]
    }

    pub fn beat_i_mut<'a>(&self, song: &'a mut Song, index: usize) -> &'a mut Beat {
        &mut song.tracks[self.track].beats[index]
    }

    pub fn seek_string(&mut self, song: &Song, dire: i16) {
        let new = self.string as i16 + dire;
        self.string = new.clamp(0, self.track(song).string_count as i16 - 1) as u16;
    }

    pub fn seek_beat(&mut self, song: &mut Song, dire: isize) {
        let new = (self.beat as isize + dire).max(0) as usize;
        let beats = self.beats_mut(song);
        while new >= beats.len() as usize {
            beats.push(beats.last().unwrap().copy_duration());
        }
        self.beat = new;
    }

    pub fn seek_scroll(&mut self, song: &Song, dire: isize) {
        let new = (self.scroll as isize + dire).max(0) as usize;
        self.scroll = new.min(self.beats(song).len() - 1);
    }

    pub fn cursor_to_scroll(&mut self, s_bwidth: usize) {
        self.beat = self.beat.clamp(self.scroll, self.scroll + s_bwidth - 1);
    }

    pub fn scroll_to_cursor(&mut self, s_bwidth: usize) {
        if self.scroll > self.beat {
            self.scroll = self.beat;
        }
        if self.scroll + s_bwidth - 1 < self.beat {
            self.scroll = self.beat - (s_bwidth - 1);
        }
    }

    pub fn set_duration(&self, song: &mut Song, dur: Duration) {
        self.beat_mut(song).dur = dur;
    }

    pub fn set_note(&self, song: &mut Song, fret: u16) {
        self.beat_mut(song).set_note(self.string, fret);
    }

    pub fn set_notes(&self, song: &mut Song, notes: Vec<Note>) {
        self.beat_mut(song).notes = notes;
    }

    pub fn clear_note(&self, song: &mut Song) {
        self.beat_mut(song).del_note(self.string);
    }

    pub fn clear_beat(&self, song: &mut Song) {
        self.beat_mut(song).notes.clear();
    }

    pub fn clear_beats(&self, song: &mut Song, count: usize) {
        let beats = self.beats_mut(song);
        for i in self.beat..self.beat + count {
            beats[i].notes.clear()
        }
    }

    pub fn delete_beat(&self, song: &mut Song) {
        self.beats_mut(song).remove(self.beat);
    }

    pub fn delete_beats(&self, song: &mut Song, count: usize) {
        self.beats_mut(song)
            .splice(self.beat..self.beat + count, []);
    }
}
