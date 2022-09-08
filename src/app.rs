use crate::{
    error::{Error, Result},
    window,
};
use crossterm::{event, style::Stylize};
use serde::{Deserialize, Serialize};
use std::fmt;

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
}

#[derive(Serialize, Deserialize)]
struct Track {
    string_count: u16,
    beats: Vec<Beat>,
}

#[derive(Serialize, Deserialize)]
struct Song {
    tracks: Vec<Track>,
}

#[derive(Clone)]
enum Typing {
    None,
    Command(String),
    NoteEdit(String),
}

impl Typing {
    fn mut_string(&mut self) -> Option<&mut String> {
        match self {
            Typing::None => None,
            Typing::Command(s) => Some(s),
            Typing::NoteEdit(s) => Some(s),
        }
    }

    fn is_none(&self) -> bool {
        match self {
            Typing::None => true,
            _ => false,
        }
    }
}

impl fmt::Display for Typing {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Typing::None => Ok(()),
            Typing::Command(text) => f.write_fmt(format_args!("cmd:{text}")),
            Typing::NoteEdit(text) => f.write_fmt(format_args!("note:{text}")),
        }
    }
}

enum Buffer {
    Empty,
    Note(Note),
    Beat(Beat),
}

impl fmt::Debug for Buffer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => write!(f, "Empty"),
            Self::Note(_) => write!(f, "Note"),
            Self::Beat(_) => write!(f, "Beat"),
        }
    }
}

pub struct App {
    should_close: bool,
    song: Song,
    sel_track: usize,
    sel_beat: usize,
    sel_string: u16,
    typing: Typing,
    typing_res: String,
    song_path: Option<String>,
    catch_copy: Option<String>,
    copy_buffer: Buffer,
}

impl App {
    pub fn new() -> Result<Self> {
        Ok(Self {
            should_close: false,
            song_path: Some("test_song.json".into()),
            song: Song { tracks: vec![] },
            sel_track: 0,
            sel_beat: 0,
            sel_string: 0,
            typing: Typing::None,
            typing_res: "".into(),
            catch_copy: None,
            copy_buffer: Buffer::Empty,
        })
    }

    fn track(&self) -> &Track {
        self.song.tracks.get(self.sel_track).unwrap()
    }

    fn track_mut(&mut self) -> &mut Track {
        self.song.tracks.get_mut(self.sel_track).unwrap()
    }

    fn draw_durations(&self, win: &mut window::Window, range: BeatRange) -> Result<()> {
        let track = self.track();
        win.moveto(0, 0)?;
        for i in range {
            win.print(format!("~ {} ", track.beats[i].dur))?;
        }
        win.print("~")?;
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
        win.print("―")?;
        Ok(())
    }

    fn gen_status_msg(&self) -> String {
        let msg = if let Some(c) = &self.catch_copy {
            format!("copy:{c}")
        } else {
            format!("buffer:{:?}", self.copy_buffer)
        };
        if self.typing.is_none() {
            format!("{} | {msg}", self.typing_res)
        } else {
            format!("{} < {msg}", self.typing)
        }
    }

    fn save_file(&self, path: String) -> Result<String> {
        let s = serde_json::to_string(&self.song).unwrap();
        std::fs::write(&path, s).unwrap();
        Ok(format!("Saved to {path}"))
    }

    fn try_save_file(&self, inp: Option<&&str>) -> Result<String> {
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
            Err(Error::InvalidOp("Cannot read file '{path}'".into()))
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
        let len = (max as usize).min(self.track().beats.len());
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
        let beats = &mut self.track_mut().beats;
        while new >= beats.len() as usize {
            beats.push(beats.last().unwrap().copy_duration());
        }
        self.sel_beat = new;
    }

    fn add_track(&mut self) {
        self.song.tracks.push(Track {
            beats: vec![Beat::new(Duration::Whole)],
            string_count: 6,
        });
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
                Some(_) => Err(Error::UnknownCmd(s_cmd)),
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
                        let beat = self.sel_beat;
                        let string = self.sel_string;
                        self.track_mut().beats[beat].set_note(string, fret);
                        Ok(format!(
                            "Note count : {}",
                            self.track_mut().beats[beat].notes.len()
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
            let beat = self.sel_beat;
            let string = self.sel_string;
            self.track_mut().beats[beat].set_note(string, fret);
            Ok(format!("Set fret ({fret})"))
        } else {
            Err(Error::MalformedCmd(format!(
                "Cannot parse {s_fret:?} as int"
            )))
        }
    }

    fn process_typing(&mut self) -> Result<()> {
        let res = match self.typing.clone() {
            Typing::Command(s) => self.process_command(s),
            Typing::NoteEdit(s) => self.process_note_edit(s),
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
                let beat = self.sel_beat;
                let string = self.sel_string;
                let fret = note.fret;
                self.track_mut().beats[beat].set_note(string, fret);
            }
            Buffer::Beat(beat) => {
                let index = self.sel_beat;
                let beat = beat.clone();
                if in_place {
                    self.track_mut().beats[index] = beat;
                } else {
                    self.track_mut().beats.insert(index, beat);
                }
            }
        }
    }

    fn key_press(&mut self, key: event::KeyCode) {
        if self.catch_copy.is_some() {
            match key {
                event::KeyCode::Char('n') => {
                    let beat = &self.track().beats[self.sel_beat];
                    if let Some(note) = beat.get_note(self.sel_string) {
                        self.copy_buffer = Buffer::Note(note.clone());
                    } else {
                        self.copy_buffer = Buffer::Empty;
                    }
                    self.catch_copy = None;
                }
                event::KeyCode::Char('b') => {
                    let beat = &self.track().beats[self.sel_beat];
                    self.copy_buffer = Buffer::Beat(beat.clone());
                    self.catch_copy = None;
                }
                event::KeyCode::Char(c) => {
                    if c.is_digit(10) {
                        self.catch_copy.as_mut().unwrap().push(c);
                    }
                }
                _ => {}
            }
        } else if let Some(typing) = self.typing.mut_string() {
            match key {
                event::KeyCode::Char(c) => typing.push(c),
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
                event::KeyCode::Char('n') => self.typing = Typing::NoteEdit(String::new()),
                event::KeyCode::Char('c') => self.catch_copy = Some(String::new()),
                event::KeyCode::Char('v') => self.paste_once(false),
                event::KeyCode::Char('V') => self.paste_once(true),
                event::KeyCode::Char(' ') => {
                    self.typing = Typing::Command(String::with_capacity(16))
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
                event::Event::Resize(..) => Ok(true),
                _ => Ok(false),
            },
            Err(Error::NoEvent) => Ok(false),
            Err(e) => Err(e),
        }
    }

    pub fn run(mut self) -> Result<()> {
        self.try_load_file(None)?;
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
