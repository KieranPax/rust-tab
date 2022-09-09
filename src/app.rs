use crate::{
    error::{Error, Result},
    window,
};
use clap::Parser;
use crossterm::{event, style::Stylize};
use serde::{Deserialize, Serialize};
use std::fmt;

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

#[derive(Clone, Copy, Debug, serde_repr::Serialize_repr, serde_repr::Deserialize_repr)]
#[repr(u8)]
enum Duration {
    Whole,
    Half,
    Quarter,
    Eighth,
    Sixteenth,
    ThirtyTwoth,
}

impl Duration {
    fn split(&self) -> Result<Self> {
        match self {
            Self::Whole => Ok(Self::Half),
            Self::Half => Ok(Self::Quarter),
            Self::Quarter => Ok(Self::Eighth),
            Self::Eighth => Ok(Self::Sixteenth),
            Self::Sixteenth => Ok(Self::ThirtyTwoth),
            _ => Err(Error::InvalidOp(format!(
                "Cannot split duration ({self:?})"
            ))),
        }
    }
}

impl fmt::Display for Duration {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Duration::Whole => f.write_str(" "),
            Duration::Half => f.write_str("-"),
            Duration::Quarter => f.write_str("1"),
            Duration::Eighth => f.write_str("2"),
            Duration::Sixteenth => f.write_str("3"),
            Duration::ThirtyTwoth => f.write_str("4"),
        }
    }
}

impl std::str::FromStr for Duration {
    type Err = Error;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s {
            "1" => Ok(Self::Whole),
            "2" => Ok(Self::Half),
            "4" => Ok(Self::Quarter),
            "8" => Ok(Self::Eighth),
            "16" => Ok(Self::Sixteenth),
            "32" => Ok(Self::ThirtyTwoth),
            _ => Err(Error::InvalidOp(format!("Cannot parse '{s}' as Duration"))),
        }
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
            beats: vec![Beat::new(Duration::Whole)],
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
}

impl Typing {
    fn mut_string(&mut self) -> Option<&mut String> {
        match self {
            Typing::None => None,
            Typing::Command(s)
            | Typing::Note(s)
            | Typing::Copy(s)
            | Typing::Delete(s)
            | Typing::Duration(s) => Some(s),
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
            Typing::Copy(_) | Typing::Delete(_) => true,
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

pub struct App {
    should_close: bool,
    song_path: Option<String>,
    song: Song,
    sel_track: usize,
    sel_beat: usize,
    sel_string: u16,
    typing: Typing,
    typing_res: String,
    copy_buffer: Buffer,
}

impl App {
    pub fn new() -> Result<Self> {
        Ok(Self {
            should_close: false,
            song_path: Some("test_song.json".into()),
            song: Song::new(),
            sel_track: 0,
            sel_beat: 0,
            sel_string: 0,
            typing: Typing::None,
            typing_res: String::new(),
            copy_buffer: Buffer::Empty,
        })
    }

    fn track(&self) -> &Track {
        &self.song.tracks[self.sel_track]
    }

    fn track_mut(&mut self) -> &mut Track {
        &mut self.song.tracks[self.sel_track]
    }

    fn beats(&self) -> &Vec<Beat> {
        &self.song.tracks[self.sel_track].beats
    }

    fn beats_mut(&mut self) -> &mut Vec<Beat> {
        &mut self.song.tracks[self.sel_track].beats
    }

    fn beat(&self) -> &Beat {
        &self.song.tracks[self.sel_track].beats[self.sel_beat]
    }

    fn beat_mut(&mut self) -> &mut Beat {
        &mut self.song.tracks[self.sel_track].beats[self.sel_beat]
    }

    fn draw_durations(&self, win: &mut window::Window, range: BeatRange) -> Result<()> {
        let track = self.track();
        win.moveto(0, 0)?;
        for i in range {
            win.print(format!("~ {} ", track.beats[i].dur))?;
        }
        win.print("~")?.clear_eoline()?;
        Ok(())
    }

    fn draw_string(&self, win: &mut window::Window, string: u16, range: BeatRange) -> Result<()> {
        let track = self.track();
        let sel_string = self.sel_string == string;
        win.moveto(0, string + 1)?;
        for i in range {
            win.print("―")?;
            let inner: String = if let Some(val) = track.beats[i].get_note(string) {
                if val.fret > 999 {
                    "###".into()
                } else {
                    format!("{: ^3}", val.fret)
                }
            } else {
                "―――".into()
            };
            if self.sel_beat as usize == i {
                win.print_styled(if sel_string {
                    inner.as_str().on_white().black()
                } else {
                    inner.as_str().on_dark_grey().black()
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
        let start = 0;
        let len = (max as usize).min(self.beats().len());
        start..start + len
    }

    fn draw(&self, win: &mut window::Window, (w, _h): (u16, u16)) -> Result<()> {
        let track = self.track();
        win.moveto(0, 0)?;
        let range = self.visible_beat_range(w / 4);
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
        let new = self.sel_string as i16 + dire;
        self.sel_string = new.clamp(0, self.track().string_count as i16 - 1) as u16;
    }

    fn seek_beat(&mut self, dire: isize) {
        let new = (self.sel_beat as isize + dire).max(0) as usize;
        let beats = self.beats_mut();
        while new >= beats.len() as usize {
            beats.push(beats.last().unwrap().copy_duration());
        }
        self.sel_beat = new;
    }

    fn add_track(&mut self) {
        self.song.tracks.push(Track::new());
    }

    fn process_command(&mut self, s_cmd: String) -> Result<String> {
        let cmd: Vec<_> = s_cmd.split(' ').collect();
        match cmd[0] {
            "" => Ok(String::new()),
            "t" => match cmd.get(1) {
                Some(&"add") => {
                    self.add_track();
                    Ok(format!("Added track [{}]", self.song.tracks.len() - 1))
                }
                Some(s) => {
                    if let Ok(index) = s.parse() {
                        self.sel_track = index;
                        Ok(format!("Switched to track [{index}]"))
                    } else {
                        Err(Error::UnknownCmd(s_cmd))
                    }
                }
                None => Err(Error::MalformedCmd(s_cmd)),
            },
            "b" => match cmd.get(1) {
                Some(&"split") => {
                    let index = self.sel_beat;
                    let track = self.track_mut();
                    let s_dur = track.beats[index].dur.split()?;
                    track.beats[index].dur = s_dur;
                    track.beats.insert(index, Beat::new(s_dur));
                    Ok(format!("Split beat[{index}] into {s_dur:?}"))
                }
                Some(_) => Err(Error::UnknownCmd(s_cmd)),
                None => Err(Error::MalformedCmd(s_cmd)),
            },
            "n" => match cmd.get(1) {
                Some(s) => {
                    if let Ok(fret) = s.parse::<u32>() {
                        let string = self.sel_string;
                        self.beat_mut().set_note(string, fret);
                        Ok(format!(
                            "Note count : {}",
                            self.beat().notes.len()
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

    fn process_note_edit(&mut self, s_fret: String) -> Result<String> {
        if let Ok(fret) = s_fret.parse() {
            let string = self.sel_string;
            self.beat_mut().set_note(string, fret);
            Ok(format!("Set fret ({fret})"))
        } else {
            Err(Error::MalformedCmd(format!(
                "Cannot parse {s_fret:?} as int"
            )))
        }
    }

    fn process_copy(&mut self, cmd: String) -> Result<String> {
        if cmd.len() == 0 {
            Ok(String::new())
        } else {
            let (a, b) = cmd.split_at(cmd.len() - 1);
            let a: std::result::Result<usize, _> = a.parse();
            match (a, b) {
                (_, "n") => {
                    if let Some(note) = self.beat().get_note(self.sel_string) {
                        self.copy_buffer = Buffer::Note(note.clone());
                        Ok("Note copied".into())
                    } else {
                        self.copy_buffer = Buffer::Empty;
                        Err(Error::InvalidOp("No note selected".into()))
                    }
                }
                (Ok(count), "b") => {
                    if let Some(beat) = self.beats().get(self.sel_beat..self.sel_beat + count)
                    {
                        self.copy_buffer = Buffer::MultiBeat(beat.to_owned());
                        Ok("Beat(s) copied".into())
                    } else {
                        Err(Error::InvalidOp("Copy range out of range".into()))
                    }
                }
                (_, "b") => {
                    self.copy_buffer = Buffer::Beat(self.beat().clone());
                    Ok("Beat copied".into())
                }
                _ => Err(Error::MalformedCmd(format!("Unknown copy type ({b})"))),
            }
        }
    }

    fn process_delete(&mut self, cmd: String) -> Result<String> {
        if cmd.len() == 0 {
            Ok(String::new())
        } else {
            let (a, b) = cmd.split_at(cmd.len() - 1);
            let a: std::result::Result<usize, _> = a.parse();
            match (a, b) {
                (_, "n") => {
                    let string = self.sel_string;
                    self.beat_mut().del_note(string);
                    Ok("Note deleted".into())
                }
                (Ok(count), "b") => {
                    let beat = self.sel_beat;
                    self.beats_mut().splice(beat..beat + count, []);
                    Ok("Beat deleted".into())
                }
                (_, "b") => {
                    let beat = self.sel_beat;
                    self.beats_mut().remove(beat);
                    Ok("Beat deleted".into())
                }
                _ => Err(Error::MalformedCmd(format!("Unknown copy type ({b})"))),
            }
        }
    }

    fn process_duration(&mut self, cmd: String) -> Result<String> {
        let dur: Duration = cmd.parse()?;
        self.beat_mut().dur = dur;
        Ok(format!("{dur:?}"))
    }

    fn process_typing(&mut self) -> Result<()> {
        let res = match self.typing.clone() {
            Typing::Command(s) => self.process_command(s),
            Typing::Note(s) => self.process_note_edit(s),
            Typing::Copy(s) => self.process_copy(s),
            Typing::Delete(s) => self.process_delete(s),
            Typing::Duration(s) => self.process_duration(s),
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
                let string = self.sel_string;
                let fret = note.fret;
                self.beat_mut().set_note(string, fret);
            }
            Buffer::Beat(beat) => {
                let index = self.sel_beat;
                let beat = beat.clone();
                if in_place {
                    self.beats_mut()[index] = beat;
                } else {
                    self.beats_mut().insert(index, beat);
                }
            }
            Buffer::MultiBeat(beats) => {
                let index = self.sel_beat;
                let src = beats.clone();
                if in_place {
                    self.beats_mut().remove(index);
                }
                let dest = self.beats_mut();
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
                event::KeyCode::Char('v') => self.paste_once(false),
                event::KeyCode::Char('V') => self.paste_once(true),
                event::KeyCode::Char(':') => self.typing = Typing::Command(String::new()),
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
                event::Event::Resize(..) => Ok(true),
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
                self.draw(&mut win, crossterm::terminal::size().unwrap())?;
            }
            do_redraw = self.proc_event(&mut win)?;
        }
        win.clear()?.update()?;
        self.try_save_file(None)?;
        Ok(())
    }
}
