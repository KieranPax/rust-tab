use crate::{
    cursor::Cursor,
    error::Result,
    song::{Note, Song},
    window,
};
use crossterm::style::Stylize;

pub struct Lane {
    pub cur: Cursor,
}

impl Lane {
    pub fn new() -> Self {
        Self { cur: Cursor::new() }
    }

    pub fn new_t(track: usize) -> Self {
        let mut cur = Cursor::new();
        cur.track = track;
        Self { cur }
    }

    fn draw_durations(
        &self,
        win: &mut window::Window,
        range: std::ops::Range<usize>,
        song: &Song,
    ) -> Result<()> {
        let track = self.cur.track(song);
        for i in range {
            win.print("~")?.print(track.beats[i].dur.dur_icon())?;
        }
        win.print("~")?.next_line()?;
        Ok(())
    }

    fn draw_string(
        &self,
        win: &mut window::Window,
        string: u16,
        range: std::ops::Range<usize>,
        song: &Song,
        is_curr: bool,
    ) -> Result<()> {
        let track = self.cur.track(song);
        for i in range {
            win.print(if track.measure_i[i] { "|" } else { "―" })?;
            let inner = match track.beats[i].get_note(string) {
                Some(Note::Fret(fret)) if fret > &999 => "###".into(),
                Some(Note::Fret(fret)) => format!("{: ^3}", fret),
                Some(Note::X) => " X ".into(),
                None => "―――".into(),
            };
            if self.cur.beat == i {
                win.print_styled(match (is_curr, self.cur.string == string) {
                    (true, true) => inner.as_str().on_white().black(),
                    (true, false) => inner.as_str().on_grey().black(),
                    _ => inner.as_str().on_dark_grey().black(),
                })?;
            } else {
                win.print(inner)?;
            }
        }
        win.print("―")?.next_line()?;
        Ok(())
    }

    pub fn draw(
        &self,
        win: &mut window::Window,
        s_bwidth: usize,
        song: &Song,
        is_curr: bool,
    ) -> Result<()> {
        let track = self.cur.track(song);
        let num_beats = track.beats.len();
        let range = self.cur.scroll..(self.cur.scroll + s_bwidth).min(num_beats);
        self.draw_durations(win, range.clone(), song)?;
        for i in 0..track.string_count {
            self.draw_string(win, i, range.clone(), song, is_curr)?;
        }
        win.next_line()?;
        Ok(())
    }
}
