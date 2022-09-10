use crate::{
    error::{Error, Result},
    window,
};
use clap::Parser;
use crossterm::{event, style::Stylize};
use serde::{
    de::{self, Visitor},
    ser::SerializeTuple,
    Deserialize, Serialize,
};
use std::fmt;

type Fraction = fraction::GenericFraction<u8>;

#[derive(Parser, Debug)]
#[clap(name = "tab")]
#[clap(version, about, long_about = None)]
struct Args {
    #[clap(value_parser)]
    path: Option<String>,
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

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd)]
struct Duration(Fraction);

impl Duration {
    fn new(a: u8, b: u8) -> Self {
        Self(Fraction::new(a, b))
    }

    fn tuple(&self) -> (u8, u8) {
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
            (1, 1) => f.write_str("   "),
            (1, 2) => f.write_str(" - "),
            (1, 4) => f.write_str(" 1 "),
            (1, 8) => f.write_str(" 2 "),
            (1, 16) => f.write_str(" 4 "),
            (1, 32) => f.write_str(" 8 "),
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
            _ => Err(Error::InvalidOp(format!("Cannot parse '{s}' as Duration"))),
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
    Clean(String),
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
            | Typing::Clean(s) => Some(s),
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
            Typing::Copy(_) | Typing::Delete(_) | Typing::Clean(_) => true,
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
            Typing::Clean(text) => f.write_fmt(format_args!("clean:{text}")),
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
}

pub struct App {
    should_close: bool,
    song_path: Option<String>,
    song: Song,
    sel: Selected,
    typing: Typing,
    typing_res: String,
    copy_buffer: Buffer,
    measure_indices: Vec<usize>,
}

impl App {
    pub fn new() -> Result<Self> {
        Ok(Self {
            should_close: false,
            song_path: None,
            song: Song::new(),
            sel: Selected::new(),
            typing: Typing::None,
            typing_res: String::new(),
            copy_buffer: Buffer::Empty,
            measure_indices: Vec::new(),
        })
    }

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

    fn visible_beat_range(&self, max: u16) -> BeatRange {
        let num_beats = self.sel.beats(&self.song).len();
        self.sel.scroll..(self.sel.scroll + max as usize).min(num_beats)
    }

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

    fn gen_status_msg(&self) -> String {
        if self.typing.is_none() {
            format!("{} | buffer : {:?}", self.typing_res, self.copy_buffer)
        } else {
            format!("{} $ buffer : {:?}", self.typing, self.copy_buffer)
        }
    }

    fn draw(&self, win: &mut window::Window, (w, _h): (u16, u16)) -> Result<()> {
        let track = self.sel.track(&self.song);
        win.moveto(0, 0)?;
        let range = self.visible_beat_range((w - 1) / 4);
        self.draw_durations(win, range.clone())?;
        for i in 0..track.string_count {
            self.draw_string(win, i, range.clone())?;
        }
        win.moveto(0, track.string_count + 2)?
            .clear_line()?
            .print(self.gen_status_msg())?
            .update()?;
        Ok(())
    }

    fn seek_string(&mut self, dire: i16) {
        let new = self.sel.string as i16 + dire;
        self.sel.string = new.clamp(0, self.sel.track(&self.song).string_count as i16 - 1) as u16;
    }

    fn seek_beat(&mut self, dire: isize) {
        let new = (self.sel.beat as isize + dire).max(0) as usize;
        let beats = self.sel.beats_mut(&mut self.song);
        while new >= beats.len() as usize {
            beats.push(beats.last().unwrap().copy_duration());
        }
        self.sel.beat = new;
    }

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

    fn proc_t_clean(&mut self, cmd: String) -> Result<String> {
        if cmd.len() == 0 {
            Ok(String::new())
        } else {
            let (a, b) = cmd.split_at(cmd.len() - 1);
            let a: std::result::Result<usize, _> = a.parse();
            match (a, b) {
                (_, "n") => {
                    let string = self.sel.string;
                    self.sel.beat_mut(&mut self.song).del_note(string);
                    Ok("Note cleared".into())
                }
                (Ok(count), "b") => {
                    let beat = self.sel.beat;
                    let beats = self.sel.beats_mut(&mut self.song);
                    for i in beat..beat + count {
                        beats[i].notes.clear()
                    }
                    Ok("Beat cleared".into())
                }
                (_, "b") => {
                    self.sel.beat_mut(&mut self.song).notes.clear();
                    Ok("Beat cleared".into())
                }
                _ => Err(Error::MalformedCmd(format!("Unknown copy type ({b})"))),
            }
        }
    }

    fn process_typing(&mut self) -> Result<()> {
        let res = match self.typing.clone() {
            Typing::Command(s) => self.proc_t_command(s),
            Typing::Note(s) => self.proc_t_note_edit(s),
            Typing::Copy(s) => self.proc_t_copy(s),
            Typing::Delete(s) => self.proc_t_delete(s),
            Typing::Duration(s) => self.proc_t_duration(s),
            Typing::Clean(s) => self.proc_t_clean(s),
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

    fn paste_once(&mut self, in_place: bool) {
        match &self.copy_buffer {
            Buffer::Empty => {}
            Buffer::Note(note) => {
                let string = self.sel.string;
                let fret = note.fret;
                self.sel.beat_mut(&mut self.song).set_note(string, fret);
            }
            Buffer::Beat(beat) => {
                let index = self.sel.beat;
                let beat = beat.clone();
                if in_place {
                    self.sel.beats_mut(&mut self.song)[index] = beat;
                } else {
                    self.sel.beats_mut(&mut self.song).insert(index, beat);
                }
            }
            Buffer::MultiBeat(beats) => {
                let index = self.sel.beat;
                let src = beats.clone();
                if in_place {
                    self.sel.beats_mut(&mut self.song).remove(index);
                }
                let dest = self.sel.beats_mut(&mut self.song);
                let after = dest.split_off(index);
                dest.extend(src);
                dest.extend(after);
            }
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
                event::KeyCode::Char('a') => self.seek_beat(-1),
                event::KeyCode::Char('d') => self.seek_beat(1),
                event::KeyCode::Char('w') => self.seek_string(-1),
                event::KeyCode::Char('s') => self.seek_string(1),
                event::KeyCode::Char('n') => self.typing = Typing::Note(String::new()),
                event::KeyCode::Char('c') => self.typing = Typing::Copy(String::new()),
                event::KeyCode::Char('x') => self.typing = Typing::Delete(String::new()),
                event::KeyCode::Char('l') => self.typing = Typing::Duration(String::new()),
                event::KeyCode::Char('k') => self.typing = Typing::Clean(String::new()),
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
                    if let Some(v) = self.sel.scroll.checked_sub(1) {
                        self.sel.scroll = v
                    }
                }
                event::KeyCode::Right => {
                    if let Some(v) = self.sel.scroll.checked_add(1) {
                        self.sel.scroll = v
                    }
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
                    Ok(true)
                }
                _ => Ok(false),
            },
            Err(Error::NoEvent) => Ok(false),
            Err(e) => Err(e),
        }
    }

    pub fn run(mut self) -> Result<()> {
        let args = Args::parse();
        self.song_path = args.path;
        let _ = self.try_load_file(None);

        let mut win = window::Window::new()?;
        win.clear()?;
        let mut do_redraw = true;
        while !self.should_close {
            if do_redraw {
                self.reset_measure_indices();
                self.draw(&mut win, crossterm::terminal::size().unwrap())?;
            }
            do_redraw = self.proc_event(&mut win)?;
        }
        win.clear()?.update()?;
        self.try_save_file(None)?;
        Ok(())
    }
}
