use crate::{
    dur::Duration,
    error::{Error, Result},
    window,
};
use clap::Parser;
use crossterm::{event, style::Stylize};
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Parser, Debug)]
#[clap(name = "rust-tab")]
#[clap(version, about, long_about = None)]
struct Args {
    #[clap(value_parser)]
    path: Option<String>,
    #[clap(short, long, action)]
    draw_timer: bool,
}

type BeatRange = std::ops::Range<usize>;

#[derive(Clone, Serialize, Deserialize)]
struct Note {
    string: u16,
    fret: u32,
}

impl Note {
    fn new(string: u16, fret: u32) -> Self {
        Self { string, fret }
    }
}

#[derive(Clone, Serialize, Deserialize)]
struct Beat {
    dur: Duration,
    notes: Vec<Note>,
}

impl Beat {
    fn new(dur: Duration) -> Self {
        Self {
            dur,
            notes: Vec::new(),
        }
    }

    fn copy_duration(&self) -> Self {
        Self::new(self.dur)
    }

    fn get_note(&self, string: u16) -> Option<&Note> {
        for i in self.notes.iter() {
            if i.string == string {
                return Some(i);
            }
        }
        None
    }

    fn set_note(&mut self, string: u16, fret: u32) {
        for i in self.notes.iter_mut() {
            if i.string == string {
                i.fret = fret;
                return;
            }
        }
        self.notes.push(Note::new(string, fret))
    }

    fn del_note(&mut self, string: u16) {
        for i in 0..self.notes.len() {
            if self.notes[i].string == string {
                self.notes.swap_remove(i);
                return;
            }
        }
    }
}

#[derive(Serialize, Deserialize)]
struct Track {
    string_count: u16,
    beats: Vec<Beat>,
}

impl Track {
    fn new() -> Self {
        Self {
            string_count: 6,
            beats: vec![Beat::new(Duration::new(1, 1))],
        }
    }
}

#[derive(Serialize, Deserialize)]
struct Song {
    tracks: Vec<Track>,
}

impl Song {
    fn new() -> Self {
        Self {
            tracks: vec![Track::new()],
        }
    }
}

#[derive(Clone)]
enum Typing {
    None,
    Command(String),
    Note(String),
    Copy(String),
    Delete(String),
    Duration(String),
    Clear(String),
}

impl Typing {
    fn mut_string(&mut self) -> Option<&mut String> {
        match self {
            Typing::None => None,
            Typing::Command(s)
            | Typing::Note(s)
            | Typing::Copy(s)
            | Typing::Delete(s)
            | Typing::Duration(s)
            | Typing::Clear(s) => Some(s),
        }
    }

    fn is_none(&self) -> bool {
        match self {
            Typing::None => true,
            _ => false,
        }
    }

    fn is_number_char(&self) -> bool {
        match self {
            Typing::Copy(_) | Typing::Delete(_) | Typing::Clear(_) => true,
            _ => false,
        }
    }
}

impl fmt::Display for Typing {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Typing::None => Ok(()),
            Typing::Command(text) => f.write_fmt(format_args!("cmd:{text}")),
            Typing::Note(text) => f.write_fmt(format_args!("note:{text}")),
            Typing::Copy(text) => f.write_fmt(format_args!("copy:{text}")),
            Typing::Delete(text) => f.write_fmt(format_args!("delete:{text}")),
            Typing::Duration(text) => f.write_fmt(format_args!("duration:{text}")),
            Typing::Clear(text) => f.write_fmt(format_args!("clean:{text}")),
        }
    }
}

enum Buffer {
    Empty,
    Note(Note),
    Beat(Beat),
    MultiBeat(Vec<Beat>),
}

impl fmt::Debug for Buffer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => write!(f, "Empty"),
            Self::Note(_) => write!(f, "Note"),
            Self::Beat(_) => write!(f, "Beat"),
            Self::MultiBeat(_) => write!(f, "MultiBeat"),
        }
    }
}

struct Selected {
    scroll: usize,
    track: usize,
    beat: usize,
    string: u16,
}

impl Selected {
    fn new() -> Self {
        Self {
            scroll: 0,
            track: 0,
            beat: 0,
            string: 0,
        }
    }

    fn track<'a>(&self, song: &'a Song) -> &'a Track {
        &song.tracks[self.track]
    }

    fn track_mut<'a>(&self, song: &'a mut Song) -> &'a mut Track {
        &mut song.tracks[self.track]
    }

    fn beats<'a>(&self, song: &'a Song) -> &'a Vec<Beat> {
        &song.tracks[self.track].beats
    }

    fn beats_mut<'a>(&self, song: &'a mut Song) -> &'a mut Vec<Beat> {
        &mut song.tracks[self.track].beats
    }

    fn beat<'a>(&self, song: &'a Song) -> &'a Beat {
        &song.tracks[self.track].beats[self.beat]
    }

    fn beat_mut<'a>(&self, song: &'a mut Song) -> &'a mut Beat {
        &mut song.tracks[self.track].beats[self.beat]
    }

    fn seek_string(&mut self, song: &Song, dire: i16) {
        let new = self.string as i16 + dire;
        self.string = new.clamp(0, self.track(song).string_count as i16 - 1) as u16;
    }

    fn seek_beat(&mut self, song: &mut Song, dire: isize) {
        let new = (self.beat as isize + dire).max(0) as usize;
        let beats = self.beats_mut(song);
        while new >= beats.len() as usize {
            beats.push(beats.last().unwrap().copy_duration());
        }
        self.beat = new;
    }

    fn seek_scroll(&mut self, song: &Song, dire: isize) {
        let new = (self.scroll as isize + dire).max(0) as usize;
        self.scroll = new.min(self.beats(song).len() - 1);
    }

    fn cursor_to_scroll(&mut self, s_bwidth: usize) {
        self.beat = self.beat.clamp(self.scroll, self.scroll + s_bwidth - 1);
    }

    fn scroll_to_cursor(&mut self, s_bwidth: usize) {
        if self.scroll > self.beat {
            self.scroll = self.beat;
        }
        if self.scroll + s_bwidth - 1 < self.beat {
            self.scroll = self.beat - (s_bwidth - 1);
        }
    }
}

pub struct App {
    args: Args,
    should_close: bool,
    song_path: Option<String>,
    song: Song,
    sel: Selected,
    typing: Typing,
    typing_res: String,
    copy_buffer: Buffer,
    measure_indices: Vec<usize>,
    s_bwidth: usize,
    s_height: u16,
}

impl App {
    pub fn new() -> Result<Self> {
        Ok(Self {
            args: Args::parse(),
            should_close: false,
            song_path: None,
            song: Song::new(),
            sel: Selected::new(),
            typing: Typing::None,
            typing_res: String::new(),
            copy_buffer: Buffer::Empty,
            measure_indices: Vec::new(),
            s_bwidth: 4,
            s_height: 4,
        })
    }

    // IO functions

    fn save_file(&mut self, path: String) -> Result<String> {
        let s = serde_json::to_string(&self.song).unwrap();
        std::fs::write(&path, s).unwrap();
        self.song_path = Some(path.clone());
        Ok(format!("Saved to {path}"))
    }

    fn try_save_file(&mut self, inp: Option<&&str>) -> Result<String> {
        if let Some(path) = inp {
            self.save_file(path.to_string())
        } else {
            if let Some(path) = self.song_path.clone() {
                self.save_file(path)
            } else {
                Err(Error::MalformedCmd("No default file to save to".into()))
            }
        }
    }

    fn load_file(&mut self, path: String) -> Result<String> {
        if let Ok(data) = std::fs::read_to_string(&path) {
            self.song = serde_json::from_str(data.as_str()).unwrap();
            Ok(format!("Loaded {path}"))
        } else {
            Err(Error::InvalidOp(format!("Cannot read file '{path}'")))
        }
    }

    fn try_load_file(&mut self, inp: Option<&&str>) -> Result<String> {
        if let Some(path) = inp {
            self.load_file(path.to_string())
        } else {
            if let Some(path) = self.song_path.clone() {
                self.load_file(path)
            } else {
                Err(Error::MalformedCmd("No default file to save to".into()))
            }
        }
    }

    // Copy paste functions

    fn paste_note(&mut self, string: u16, fret: u32) {
        self.sel.beat_mut(&mut self.song).set_note(string, fret);
    }

    fn paste_beat(&mut self, in_place: bool, index: usize, beat: Beat) {
        if in_place {
            self.sel.beats_mut(&mut self.song)[index] = beat;
        } else {
            self.sel.beats_mut(&mut self.song).insert(index, beat);
        }
    }

    fn paste_multi_beat(&mut self, in_place: bool, index: usize, src: Vec<Beat>) {
        if in_place {
            self.sel.beats_mut(&mut self.song).remove(index);
        }
        let dest = self.sel.beats_mut(&mut self.song);
        let after = dest.split_off(index);
        dest.extend(src);
        dest.extend(after);
    }

    fn paste_once(&mut self, in_place: bool) {
        match &self.copy_buffer {
            Buffer::Empty => {}
            Buffer::Note(n) => self.paste_note(self.sel.string, n.fret),
            Buffer::Beat(b) => self.paste_beat(in_place, self.sel.beat, b.clone()),
            Buffer::MultiBeat(b) => self.paste_multi_beat(in_place, self.sel.beat, b.clone()),
        }
    }

    // Drawing util functions

    fn reset_measure_indices(&mut self) {
        let mut v = Vec::new();
        let mut dur = Duration::new(0, 1);
        let measure_width = Duration::new(1, 1);
        for (i, beat) in self.sel.beats(&self.song).iter().enumerate() {
            if dur == measure_width {
                v.push(i);
                dur = Duration::new(0, 1);
            } else if dur > measure_width {
                dur = dur - measure_width;
            }
            dur = dur + beat.dur;
        }
        self.measure_indices = v;
    }

    fn reset_sdim(&mut self, (w, h): (u16, u16)) {
        self.s_bwidth = ((w - 1) / 4) as usize;
        self.s_height = h;
    }

    fn visible_beat_range(&self) -> BeatRange {
        let num_beats = self.sel.beats(&self.song).len();
        self.sel.scroll..(self.sel.scroll + self.s_bwidth).min(num_beats)
    }

    fn gen_status_msg(&self) -> String {
        if self.typing.is_none() {
            format!("{} | buffer : {:?}", self.typing_res, self.copy_buffer)
        } else {
            format!("{} $ buffer : {:?}", self.typing, self.copy_buffer)
        }
    }

    // Draw functions

    fn draw_durations(&self, win: &mut window::Window, range: BeatRange) -> Result<()> {
        let track = self.sel.track(&self.song);
        win.moveto(0, 0)?;
        for i in range {
            win.print(format!("~{}", track.beats[i].dur))?;
        }
        win.print("~")?.clear_eoline()?;
        Ok(())
    }

    fn draw_string(&self, win: &mut window::Window, string: u16, range: BeatRange) -> Result<()> {
        let track = self.sel.track(&self.song);
        win.moveto(0, string + 1)?;
        for i in range {
            win.print_styled(if self.measure_indices.contains(&i) {
                "|".white()
            } else {
                "―".grey()
            })?;
            let inner: String = if let Some(val) = track.beats[i].get_note(string) {
                if val.fret > 999 {
                    "###".into()
                } else {
                    format!("{: ^3}", val.fret)
                }
            } else {
                "―――".into()
            };
            if self.sel.beat == i {
                win.print_styled(if self.sel.string == string {
                    inner.as_str().on_white().black()
                } else {
                    inner.as_str().on_grey().black()
                })?;
            } else {
                win.print(inner)?;
            }
        }
        win.print("―")?.clear_eoline()?;
        Ok(())
    }

    fn draw(&self, win: &mut window::Window) -> Result<()> {
        let t0 = std::time::Instant::now();
        let track = self.sel.track(&self.song);
        win.moveto(0, 0)?;
        let range = self.visible_beat_range();
        self.draw_durations(win, range.clone())?;
        for i in 0..track.string_count {
            self.draw_string(win, i, range.clone())?;
        }
        win.moveto(0, track.string_count + 2)?
            .clear_line()?
            .print(self.gen_status_msg())?;
        let dur = std::time::Instant::now().duration_since(t0).as_secs_f32() * 1000.0;
        if self.args.draw_timer {
            win.print(format!("     -> ({dur:.2}ms)"))?;
        }
        win.update()?;
        Ok(())
    }

    // Sub-command functions

    fn clear_note(&mut self, index: usize, string: u16) {
        self.sel.beats_mut(&mut self.song)[index].del_note(string);
    }

    fn clear_beat(&mut self, index: usize) {
        self.sel.beats_mut(&mut self.song)[index].notes.clear();
    }

    fn clear_beats(&mut self, start: usize, count: usize) {
        let beats = self.sel.beats_mut(&mut self.song);
        for i in start..start + count {
            beats[i].notes.clear()
        }
    }

    // Command processors

    fn proc_t_command(&mut self, s_cmd: String) -> Result<String> {
        let cmd: Vec<_> = s_cmd.split(' ').collect();
        match cmd[0] {
            "" => Ok(String::new()),
            "t" => match cmd.get(1) {
                Some(&"add") => {
                    self.song.tracks.push(Track::new());
                    Ok(format!("Added track [{}]", self.song.tracks.len() - 1))
                }
                Some(s) => {
                    if let Ok(index) = s.parse() {
                        self.sel.track = index;
                        Ok(format!("Switched to track [{index}]"))
                    } else {
                        Err(Error::UnknownCmd(s_cmd))
                    }
                }
                None => Err(Error::MalformedCmd(s_cmd)),
            },
            "b" => match cmd.get(1) {
                Some(_) => Err(Error::UnknownCmd(s_cmd)),
                None => Err(Error::MalformedCmd(s_cmd)),
            },
            "n" => match cmd.get(1) {
                Some(s) => {
                    if let Ok(fret) = s.parse::<u32>() {
                        let string = self.sel.string;
                        self.sel.beat_mut(&mut self.song).set_note(string, fret);
                        Ok(format!(
                            "Note count : {}",
                            self.sel.beat(&self.song).notes.len()
                        ))
                    } else {
                        Err(Error::UnknownCmd(s_cmd))
                    }
                }
                None => Err(Error::MalformedCmd(s_cmd)),
            },
            "f" => match cmd.get(1) {
                Some(&"save") => self.try_save_file(cmd.get(2)),
                Some(&"load") => self.try_load_file(cmd.get(2)),
                Some(_) => Err(Error::UnknownCmd(s_cmd)),
                None => Err(Error::MalformedCmd(s_cmd)),
            },
            _ => Err(Error::UnknownCmd(s_cmd)),
        }
    }

    fn proc_t_note_edit(&mut self, s_fret: String) -> Result<String> {
        if let Ok(fret) = s_fret.parse() {
            let string = self.sel.string;
            self.sel.beat_mut(&mut self.song).set_note(string, fret);
            Ok(format!("Set fret ({fret})"))
        } else {
            Err(Error::MalformedCmd(format!(
                "Cannot parse {s_fret:?} as int"
            )))
        }
    }

    fn proc_t_copy(&mut self, cmd: String) -> Result<String> {
        if cmd.len() == 0 {
            Ok(String::new())
        } else {
            let (a, b) = cmd.split_at(cmd.len() - 1);
            let a: std::result::Result<usize, _> = a.parse();
            match (a, b) {
                (_, "n") => {
                    if let Some(note) = self.sel.beat(&self.song).get_note(self.sel.string) {
                        self.copy_buffer = Buffer::Note(note.clone());
                        Ok("Note copied".into())
                    } else {
                        self.copy_buffer = Buffer::Empty;
                        Err(Error::InvalidOp("No note selected".into()))
                    }
                }
                (Ok(count), "b") => {
                    if let Some(beat) = self
                        .sel
                        .beats(&self.song)
                        .get(self.sel.beat..self.sel.beat + count)
                    {
                        self.copy_buffer = Buffer::MultiBeat(beat.to_owned());
                        Ok("Beat(s) copied".into())
                    } else {
                        Err(Error::InvalidOp("Copy range out of range".into()))
                    }
                }
                (_, "b") => {
                    self.copy_buffer = Buffer::Beat(self.sel.beat(&self.song).clone());
                    Ok("Beat copied".into())
                }
                _ => Err(Error::MalformedCmd(format!("Unknown copy type ({b})"))),
            }
        }
    }

    fn proc_t_delete(&mut self, cmd: String) -> Result<String> {
        if cmd.len() == 0 {
            Ok(String::new())
        } else {
            let (a, b) = cmd.split_at(cmd.len() - 1);
            let a: std::result::Result<usize, _> = a.parse();
            match (a, b) {
                (_, "n") => {
                    let string = self.sel.string;
                    self.sel.beat_mut(&mut self.song).del_note(string);
                    Ok("Note deleted".into())
                }
                (Ok(count), "b") => {
                    let beat = self.sel.beat;
                    self.sel
                        .beats_mut(&mut self.song)
                        .splice(beat..beat + count, []);
                    Ok("Beat deleted".into())
                }
                (_, "b") => {
                    let beat = self.sel.beat;
                    self.sel.beats_mut(&mut self.song).remove(beat);
                    Ok("Beat deleted".into())
                }
                _ => Err(Error::MalformedCmd(format!("Unknown copy type ({b})"))),
            }
        }
    }

    fn proc_t_duration(&mut self, cmd: String) -> Result<String> {
        let dur: Duration = cmd.parse()?;
        self.sel.beat_mut(&mut self.song).dur = dur;
        Ok(format!("{dur:?}"))
    }

    fn proc_t_clear(&mut self, cmd: String) -> Result<String> {
        if cmd.len() == 0 {
            Ok(String::new())
        } else {
            let (a, b) = cmd.split_at(cmd.len() - 1);
            let a: std::result::Result<usize, _> = a.parse();
            match (a, b) {
                (_, "n") => {
                    self.clear_note(self.sel.beat, self.sel.string);
                    Ok("Note cleared".into())
                }
                (Ok(count), "b") => {
                    self.clear_beats(self.sel.beat, count);
                    Ok("{count} Beats cleared".into())
                }
                (_, "b") => {
                    self.clear_beat(self.sel.beat);
                    Ok("Beat cleared".into())
                }
                _ => Err(Error::MalformedCmd(format!("Unknown copy type ({b})"))),
            }
        }
    }

    // Raw event processors

    fn process_typing(&mut self) -> Result<()> {
        let res = match self.typing.clone() {
            Typing::Command(s) => self.proc_t_command(s),
            Typing::Note(s) => self.proc_t_note_edit(s),
            Typing::Copy(s) => self.proc_t_copy(s),
            Typing::Delete(s) => self.proc_t_delete(s),
            Typing::Duration(s) => self.proc_t_duration(s),
            Typing::Clear(s) => self.proc_t_clear(s),
            Typing::None => panic!("App.typing hasn't been initiated"),
        };
        if let Err(e) = res {
            self.typing = Typing::None;
            self.typing_res = format!("{e:?}");
            Ok(())
        } else {
            self.typing = Typing::None;
            self.typing_res = res.unwrap();
            Ok(())
        }
    }

    fn key_press(&mut self, key: event::KeyCode) {
        if let Some(typing) = self.typing.mut_string() {
            match key {
                event::KeyCode::Char(c) => {
                    typing.push(c);
                    if self.typing.is_number_char() && !c.is_digit(10) {
                        self.process_typing().unwrap()
                    }
                }
                event::KeyCode::Enter => self.process_typing().unwrap(),
                event::KeyCode::Backspace => {
                    typing.pop();
                }
                _ => {}
            }
        } else {
            match key {
                event::KeyCode::Char('q') | event::KeyCode::Esc => self.should_close = true,
                event::KeyCode::Char('a') => {
                    self.sel.seek_beat(&mut self.song, -1);
                    self.sel.scroll_to_cursor(self.s_bwidth)
                }
                event::KeyCode::Char('d') => {
                    self.sel.seek_beat(&mut self.song, 1);
                    self.sel.scroll_to_cursor(self.s_bwidth)
                }
                event::KeyCode::Char('w') => self.sel.seek_string(&self.song, -1),
                event::KeyCode::Char('s') => self.sel.seek_string(&self.song, 1),
                event::KeyCode::Char('n') => self.typing = Typing::Note(String::new()),
                event::KeyCode::Char('c') => self.typing = Typing::Copy(String::new()),
                event::KeyCode::Char('x') => self.typing = Typing::Delete(String::new()),
                event::KeyCode::Char('l') => self.typing = Typing::Duration(String::new()),
                event::KeyCode::Char('k') => self.typing = Typing::Clear(String::new()),
                event::KeyCode::Char('v') => self.paste_once(false),
                event::KeyCode::Char('V') => self.paste_once(true),
                event::KeyCode::Char(':') => self.typing = Typing::Command(String::new()),
                event::KeyCode::Char('i') => {
                    let beat = self.sel.beat(&self.song).copy_duration();
                    self.sel
                        .beats_mut(&mut self.song)
                        .insert(self.sel.beat, beat);
                }
                event::KeyCode::Left => {
                    self.sel.seek_scroll(&self.song, -1);
                    self.sel.cursor_to_scroll(self.s_bwidth)
                }
                event::KeyCode::Right => {
                    self.sel.seek_scroll(&self.song, 1);
                    self.sel.cursor_to_scroll(self.s_bwidth)
                }
                _ => {}
            }
        }
    }

    fn proc_event(&mut self, win: &mut window::Window) -> Result<bool> {
        match win.get_event() {
            Ok(e) => match e {
                event::Event::Key(e) => match e {
                    event::KeyEvent { code, .. } => {
                        self.key_press(code);
                        Ok(true)
                    }
                },
                event::Event::Resize(..) => {
                    win.moveto(0, 0)?.clear()?;
                    self.reset_sdim(crossterm::terminal::size().unwrap());
                    Ok(true)
                }
                _ => Ok(false),
            },
            Err(Error::NoEvent) => Ok(false),
            Err(e) => Err(e),
        }
    }

    // Main loop

    pub fn run(mut self) -> Result<()> {
        self.song_path = self.args.path.clone();
        let _ = self.try_load_file(None);

        let mut win = window::Window::new()?;
        win.clear()?;
        self.reset_sdim(crossterm::terminal::size().unwrap());
        let mut do_redraw = true;
        while !self.should_close {
            if do_redraw {
                self.reset_measure_indices();
                self.draw(&mut win)?;
            }
            do_redraw = self.proc_event(&mut win)?;
        }
        win.clear()?.update()?;
        self.try_save_file(None)?;
        Ok(())
    }
}
