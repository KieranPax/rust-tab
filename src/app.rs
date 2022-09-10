use crate::{
    buffer::Buffer,
    cursor::Cursor,
    dur::Duration,
    error::{Error, Result, SResult},
    history::{Action, History},
    song::{Song, Track},
    window,
};
use clap::Parser;
use crossterm::{
    event::{self, KeyCode},
    style::Stylize,
};
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

pub struct App {
    args: Args,
    should_close: bool,
    song_path: Option<String>,
    song: Song,
    sel: Cursor,
    typing: Typing,
    typing_res: String,
    copy_buf: Buffer,
    s_bwidth: usize,
    s_height: u16,
    history: History,
}

impl App {
    pub fn new() -> Result<Self> {
        Ok(Self {
            args: Args::parse(),
            should_close: false,
            song_path: None,
            song: Song::new(),
            sel: Cursor::new(),
            typing: Typing::None,
            typing_res: String::new(),
            copy_buf: Buffer::Empty,
            s_bwidth: 4,
            s_height: 4,
            history: History::new(32),
        })
    }

    // History functions

    fn undo(&mut self) -> Result<String> {
        if let Some(action) = self.history.undo() {
            self.undo_action(action)
        } else {
            Err(Error::InvalidOp("Cannot undo any further".into()))
        }
    }

    fn redo(&mut self) -> Result<String> {
        if let Some(action) = self.history.redo() {
            self.apply_action(action)
        } else {
            Err(Error::InvalidOp("Cannot redo any further".into()))
        }
    }

    fn apply_action(&mut self, action: std::rc::Rc<Action>) -> Result<String> {
        match &*action {
            Action::SetDuration { cur, new, .. } => {
                cur.set_duration(&mut self.song, *new);
                Ok(format!("Set duration {}/{}", new.num(), new.dem()))
            }
            Action::SetNote { cur, new, .. } => {
                if let Some(fret) = *new {
                    cur.set_note(&mut self.song, fret);
                    Ok("Set note".into())
                } else {
                    cur.clear_note(&mut self.song);
                    Ok("Delete note".into())
                }
            }
            Action::ClearBeat { cur, .. } => {
                cur.clear_beat(&mut self.song);
                Ok("Clear beat".into())
            }
            Action::DeleteBeat { cur, .. } => {
                cur.delete_beat(&mut self.song);
                Ok("Delete beat".into())
            }
        }
    }

    fn undo_action(&mut self, action: std::rc::Rc<Action>) -> Result<String> {
        match &*action {
            Action::SetDuration { cur, old, .. } => {
                cur.set_duration(&mut self.song, *old);
                Ok("Undo set duration".into())
            }
            Action::SetNote { cur, old, new } => {
                if let Some(fret) = *old {
                    cur.set_note(&mut self.song, fret);
                } else {
                    cur.clear_note(&mut self.song);
                }
                if new.is_none() {
                    Ok("Undo delete note".into())
                } else {
                    Ok("Undo set note".into())
                }
            }
            Action::ClearBeat { cur, old } => {
                cur.set_notes(&mut self.song, old.clone());
                Ok("Undo clear beat".into())
            }
            Action::DeleteBeat { cur, old } => {
                cur.paste_beat(&mut self.song, false, old.clone());
                Ok("Undo delete beat".into())
            }
        }
    }

    fn push_action(&mut self, action: Action) -> Result<String> {
        let action = std::rc::Rc::new(action);
        let res = self.apply_action(action.clone());
        if res.is_ok() {
            self.history.push(action);
        }
        res
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

    // Drawing util functions

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
            format!("{} | buffer : {:?}", self.typing_res, self.copy_buf)
        } else {
            format!("{} $ buffer : {:?}", self.typing, self.copy_buf)
        }
    }

    fn set_typing_res<T>(&mut self, res: Result<T>)
    where
        T: Into<String>,
    {
        if let Err(e) = res {
            self.typing = Typing::None;
            self.typing_res = format!("{e}");
        } else {
            self.typing = Typing::None;
            self.typing_res = res.unwrap().into();
        }
    }

    // Draw functions

    fn draw_durations(&self, win: &mut window::Window, range: BeatRange) -> Result<()> {
        let track = self.sel.track(&self.song);
        win.moveto(0, 0)?;
        for i in range {
            win.print("~")?.print(track.beats[i].dur.dur_icon())?;
        }
        win.print("~")?.clear_eoline()?;
        Ok(())
    }

    fn draw_string(&self, win: &mut window::Window, string: u16, range: BeatRange) -> Result<()> {
        let track = self.sel.track(&self.song);
        win.moveto(0, string + 1)?;
        for i in range {
            win.print_styled(if track.is_measure_start(&i) {
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
                    if let Ok(fret) = s.parse::<u16>() {
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
            self.push_action(Action::set_note(
                self.sel.clone(),
                self.sel
                    .beat(&self.song)
                    .get_note(self.sel.string)
                    .map(|n| n.fret),
                Some(fret),
            ))
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
            let a: SResult<usize, _> = a.parse();
            match (a, b) {
                (_, "n") => {
                    self.copy_buf = self.sel.copy_note(&mut self.song, self.sel.string);
                    match &self.copy_buf {
                        Buffer::Note(_) => Ok("Note copied".into()),
                        _ => Err(Error::InvalidOp("No note selected".into())),
                    }
                }
                (Ok(count), "b") => {
                    self.copy_buf = self.sel.copy_beats(&mut self.song, count);
                    match &self.copy_buf {
                        Buffer::MultiBeat(_) => Ok(format!("{count} beats copied")),
                        _ => Err(Error::InvalidOp("Copy range out of range".into())),
                    }
                }
                (_, "b") => {
                    self.copy_buf = self.sel.copy_beat(&mut self.song);
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
            let a: SResult<usize, _> = a.parse();
            match (a, b) {
                (_, "n") => self.push_action(Action::set_note(
                    self.sel.clone(),
                    self.sel
                        .beat(&self.song)
                        .get_note(self.sel.string)
                        .map(|n| n.fret),
                    None,
                )),
                (Ok(count), "b") => {
                    self.sel.delete_beats(&mut self.song, count);
                    Ok(format!("{count} beats deleted"))
                }
                (_, "b") => self.push_action(Action::delete_beat(
                    self.sel.clone(),
                    self.sel.beat(&self.song).clone(),
                )),
                _ => Err(Error::MalformedCmd(format!("Unknown copy type ({b})"))),
            }
        }
    }

    fn proc_t_duration(&mut self, cmd: String) -> Result<String> {
        let dur = Duration::parse(&cmd)?;
        self.push_action(Action::set_duration(
            self.sel.clone(),
            self.sel.beat(&self.song).dur,
            dur,
        ))
    }

    fn proc_t_clear(&mut self, cmd: String) -> Result<String> {
        if cmd.len() == 0 {
            Ok(String::new())
        } else {
            let (a, b) = cmd.split_at(cmd.len() - 1);
            let a: SResult<usize, _> = a.parse();
            match (a, b) {
                (_, "n") => self.push_action(Action::set_note(
                    self.sel.clone(),
                    self.sel
                        .beat(&self.song)
                        .get_note(self.sel.string)
                        .map(|n| n.fret),
                    None,
                )),
                (Ok(count), "b") => {
                    self.sel.clear_beats(&mut self.song, count);
                    Ok("{count} beats cleared".into())
                }
                (_, "b") => self.push_action(Action::clear_beat(
                    self.sel.clone(),
                    self.sel.beat(&self.song).notes.clone(),
                )),
                _ => Err(Error::MalformedCmd(format!("Unknown copy type ({b})"))),
            }
        }
    }

    // Raw event processors

    fn process_typing(&mut self) {
        let res = match self.typing.clone() {
            Typing::Command(s) => self.proc_t_command(s),
            Typing::Note(s) => self.proc_t_note_edit(s),
            Typing::Copy(s) => self.proc_t_copy(s),
            Typing::Delete(s) => self.proc_t_delete(s),
            Typing::Duration(s) => self.proc_t_duration(s),
            Typing::Clear(s) => self.proc_t_clear(s),
            Typing::None => panic!("App.typing hasn't been initiated"),
        };
        self.set_typing_res(res);
    }

    fn key_press(&mut self, key: KeyCode) {
        if let Some(typing) = self.typing.mut_string() {
            match key {
                KeyCode::Char(c) => {
                    typing.push(c);
                    if self.typing.is_number_char() && !c.is_digit(10) {
                        self.process_typing()
                    }
                }
                KeyCode::Enter => self.process_typing(),
                KeyCode::Backspace => {
                    typing.pop();
                }
                _ => {}
            }
        } else {
            match key {
                KeyCode::Char('q') | KeyCode::Esc => self.should_close = true,
                KeyCode::Char('a') => {
                    self.sel.seek_beat(&mut self.song, -1);
                    self.sel.scroll_to_cursor(self.s_bwidth)
                }
                KeyCode::Char('d') => {
                    self.sel.seek_beat(&mut self.song, 1);
                    self.sel.scroll_to_cursor(self.s_bwidth)
                }
                KeyCode::Char('w') => self.sel.seek_string(&self.song, -1),
                KeyCode::Char('s') => self.sel.seek_string(&self.song, 1),
                KeyCode::Char('n') => self.typing = Typing::Note(String::new()),
                KeyCode::Char('c') => self.typing = Typing::Copy(String::new()),
                KeyCode::Char('x') => self.typing = Typing::Delete(String::new()),
                KeyCode::Char('l') => self.typing = Typing::Duration(String::new()),
                KeyCode::Char('k') => self.typing = Typing::Clear(String::new()),
                KeyCode::Char('v') => self.sel.paste_once(&mut self.song, &self.copy_buf, false),
                KeyCode::Char('V') => self.sel.paste_once(&mut self.song, &self.copy_buf, true),
                KeyCode::Char('z') => {
                    let res = self.undo();
                    self.set_typing_res(res);
                }
                KeyCode::Char('Z') => {
                    let res = self.redo();
                    self.set_typing_res(res);
                }
                KeyCode::Char(':') => self.typing = Typing::Command(String::new()),
                KeyCode::Char('i') => {
                    let beat = self.sel.beat(&self.song).copy_duration();
                    self.sel
                        .beats_mut(&mut self.song)
                        .insert(self.sel.beat, beat);
                }
                KeyCode::Left => {
                    self.sel.seek_scroll(&self.song, -1);
                    self.sel.cursor_to_scroll(self.s_bwidth)
                }
                KeyCode::Right => {
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
        self.sel.track_mut(&mut self.song).update_measures();
        while !self.should_close {
            if do_redraw {
                self.draw(&mut win)?;
            }
            do_redraw = self.proc_event(&mut win)?;
        }
        win.clear()?.update()?;
        self.try_save_file(None)?;
        Ok(())
    }
}
