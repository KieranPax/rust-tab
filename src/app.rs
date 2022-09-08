use crate::{
    error::{Error, Result},
    window::{self, Color},
};
use crossterm::event;
use serde::{Deserialize, Serialize};
use std::fmt;

type BeatRange = std::ops::Range<usize>;

#[derive(Serialize, Deserialize)]
struct Note {
    string: u16,
    fret: i32,
}

impl Note {
    fn new(string: u16, fret: i32) -> Self {
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

#[derive(Serialize, Deserialize)]
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

    fn set_note(&mut self, string: u16, fret: i32) {
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
}

impl Typing {
    fn mut_string(&mut self) -> Option<&mut String> {
        match self {
            Typing::None => None,
            Typing::Command(s) => Some(s),
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
        }
    }
}

fn load_test_file() -> Option<Song> {
    let path: std::path::PathBuf = "test_song.json".into();
    if path.is_file() {
        Some(serde_json::from_str(std::fs::read_to_string(path).unwrap().as_str()).unwrap())
    } else {
        None
    }
}

fn save_test_file(song: &Song) {
    let path: std::path::PathBuf = "test_song.json".into();
    let s = serde_json::to_string(song).unwrap();
    std::fs::write(path, s).unwrap();
}

pub struct App {
    should_close: bool,
    song: Song,
    last_cursor_y: u16,
    sel_track: usize,
    sel_beat: usize,
    sel_string: u16,
    typing: Typing,
    typing_res: String,
}

impl App {
    pub fn new() -> Result<Self> {
        let song = load_test_file()
            .or_else(|| {
                Some(Song {
                    tracks: vec![Track {
                        string_count: 6,
                        beats: vec![
                            Beat::new(Duration::Quarter),
                            Beat::new(Duration::Quarter),
                            Beat::new(Duration::Quarter),
                            Beat::new(Duration::Quarter),
                        ],
                    }],
                })
            })
            .unwrap();
        Ok(Self {
            should_close: false,
            song,
            last_cursor_y: 0,
            sel_track: 0,
            sel_beat: 0,
            sel_string: 0,
            typing: Typing::None,
            typing_res: "".into(),
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
            win.print(format!("+ {} ", track.beats[i].dur))?;
        }
        win.print("+")?;
        Ok(())
    }

    fn draw_string(&self, win: &mut window::Window, string: u16, range: BeatRange) -> Result<()> {
        let track = self.track();
        let sel_string = self.sel_string == string;
        win.moveto(0, string + 1)?;
        for i in range {
            let inner: String = if let Some(val) = track.beats[i].get_note(string) {
                format!("{}", val.fret)
            } else {
                "   ".into()
            };
            if self.sel_beat as usize == i {
                win.print("|")?.print_color(
                    inner.as_str(),
                    if sel_string {
                        Color::WhiteBG
                    } else {
                        Color::GreyBG
                    },
                )?;
            } else {
                win.print(format!("|{inner}"))?;
            }
        }
        win.print("|")?;
        Ok(())
    }

    fn gen_status_msg(&self) -> String {
        if self.typing.is_none() {
            format!("{} |", self.typing_res)
        } else {
            format!("{} <", self.typing)
        }
    }

    fn visible_beat_range(&self, max: u16) -> BeatRange {
        let start = 0;
        let len = (max as usize).min(self.track().beats.len());
        start..start + len
    }

    fn draw(&self, win: &mut window::Window, (w, _h): (u16, u16)) -> Result<u16> {
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
        Ok(track.string_count + 3)
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
                    if let Ok(fret) = s.parse::<i32>() {
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
            _ => Err(Error::UnknownCmd(s_cmd)),
        }
    }

    fn process_typing(&mut self) -> Result<()> {
        let res = match self.typing.clone() {
            Typing::Command(s) => self.process_command(s),
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
        let mut win = window::Window::new()?;
        win.clear()?;
        let mut do_redraw = true;
        while !self.should_close {
            if do_redraw {
                self.last_cursor_y = self.draw(&mut win, crossterm::terminal::size().unwrap())?;
            }
            do_redraw = self.proc_event(&mut win)?;
        }
        win.clear()?.update()?;
        save_test_file(&self.song);
        Ok(())
    }
}
