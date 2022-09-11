use crate::{
    buffer::Buffer,
    cursor::Cursor,
    dur::Duration,
    error::{Error, Result, SResult},
    history::{Action, History},
    song::{Note, Song},
    window,
};
use clap::Parser;
use crossterm::{
    event::{self, KeyCode, KeyModifiers},
    style::Stylize,
};

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

struct Typing {
    count: String,
    cmd: String,
}

impl Typing {
    fn new() -> Self {
        Self {
            count: String::new(),
            cmd: String::new(),
        }
    }

    fn is_recieving(&self) -> bool {
        !(self.cmd.is_empty() && self.count.is_empty())
    }

    fn display(&self) -> String {
        format!("{} {}", self.count, self.cmd)
    }

    fn clear(&mut self) {
        self.count.clear();
        self.cmd.clear();
    }

    fn send_char(&mut self, c: char) {
        if self.cmd.is_empty() {
            if c.is_ascii_digit() {
                self.count.push(c);
            } else {
                self.cmd.push(c);
            }
        } else {
            self.cmd.push(c);
        }
    }

    fn backspace(&mut self) {
        if !self.cmd.is_empty() {
            self.cmd.pop();
        } else {
            self.count.pop();
        }
    }

    fn parse_count(&self) -> Option<usize> {
        self.count.parse().ok()
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
            typing: Typing::new(),
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
                if let Some(note) = new {
                    cur.set_note(&mut self.song, note.to_owned());
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
            Action::DeleteBeats { cur, old } => {
                cur.delete_beats(&mut self.song, old.len());
                Ok("Undo delete beat".into())
            }
            Action::PasteNote { cur, buf, .. } => {
                cur.set_note(&mut self.song, buf.clone());
                Ok("Paste note".into())
            }
            Action::PasteBeat { cur, old, buf } => {
                cur.insert_beat(&mut self.song, old.is_some(), buf.clone());
                Ok("Paste beat".into())
            }
            Action::PasteBeats { cur, old, buf } => {
                cur.insert_beats(&mut self.song, old.is_some(), buf.clone());
                Ok("Paste beats".into())
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
                if let Some(note) = old {
                    cur.set_note(&mut self.song, note.clone());
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
                cur.insert_beat(&mut self.song, false, old.clone());
                Ok("Undo delete beat".into())
            }
            Action::DeleteBeats { cur, old } => {
                cur.insert_beats(&mut self.song, false, old.clone());
                Ok("Undo delete beat".into())
            }
            Action::PasteNote { cur, old, .. } => {
                if let Some(note) = old {
                    cur.set_note(&mut self.song, note.clone());
                } else {
                    cur.clear_note(&mut self.song);
                }
                Ok("Undo paste note".into())
            }
            Action::PasteBeat { cur, old, .. } => {
                if let Some(beat) = old {
                    cur.insert_beat(&mut self.song, true, beat.clone());
                } else {
                    cur.delete_beat(&mut self.song);
                }
                Ok("Undo paste beat".into())
            }
            Action::PasteBeats { cur, old, buf } => {
                cur.delete_beats(&mut self.song, buf.len());
                if let Some(beat) = old {
                    cur.insert_beat(&mut self.song, false, beat.clone());
                }
                Ok("Undo paste beats".into())
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
        if self.typing.is_recieving() {
            format!(">{}< | buffer : {:?}", self.typing.display(), self.copy_buf)
        } else {
            format!("{} | buffer : {:?}", self.typing_res, self.copy_buf)
        }
    }

    fn set_typing_res<T>(&mut self, res: Result<T>)
    where
        T: Into<String>,
    {
        if let Err(e) = res {
            self.typing_res = format!("{e}");
        } else {
            self.typing_res = res.unwrap().into();
        }
        self.typing.clear();
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
            let inner = match track.beats[i].get_note(string) {
                Some(Note::Fret(fret)) if fret > &999 => "###".into(),
                Some(Note::Fret(fret)) => format!("{: ^3}", fret),
                Some(Note::X) => " X ".into(),
                None => "―――".into(),
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
        match cmd.get(0) {
            Some(&"save") => self.try_save_file(cmd.get(1)),
            Some(&"load") => self.try_load_file(cmd.get(1)),
            _ => Err(Error::UnknownCmd(s_cmd)),
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
                        Buffer::Beats(_) => Ok(format!("{count} beats copied")),
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

    fn push_paste_once(&mut self, in_place: bool) {
        let res = match &self.copy_buf {
            Buffer::Empty => Ok("".into()),
            Buffer::Note(n) => self.push_action(Action::paste_note(
                self.sel.clone(),
                self.sel.beat(&self.song).copy_note(self.sel.string),
                n.clone(),
            )),
            Buffer::Beat(b) => self.push_action(Action::paste_beat(
                self.sel.clone(),
                if in_place {
                    Some(self.sel.beat(&self.song).clone())
                } else {
                    None
                },
                b.clone(),
            )),
            Buffer::Beats(b) => self.push_action(Action::paste_beats(
                self.sel.clone(),
                if in_place {
                    Some(self.sel.beat(&self.song).clone())
                } else {
                    None
                },
                b.clone(),
            )),
        };
        self.set_typing_res(res);
    }

    fn apply_note(&mut self, note_str: &str) {
        if note_str.is_empty() {
            return;
        }
        let note = Note::parse(note_str);
        let res = match note {
            Ok(note) => self.push_action(Action::set_note(
                self.sel.clone(),
                self.sel.beat(&self.song).copy_note(self.sel.string),
                Some(note),
            )),
            Err(e) => Err(e),
        };
        self.set_typing_res(res);
    }

    fn check_typing(&mut self) {
        let cmd = &self.typing.cmd;
        if cmd.starts_with("l") {
            return;
        }
        if cmd.len() > 1 && cmd.starts_with("n") {
            let last = cmd.chars().last().unwrap();
            if !(last.is_ascii_digit() || last == 'x') {
                let s = cmd.get(1..cmd.len() - 1).unwrap().to_owned();
                self.apply_note(&s);
                self.typing.clear();
                match last {
                    'n' => {
                        self.sel.seek_beat(&mut self.song, 1);
                        self.typing.send_char('n');
                    }
                    'd' => self.sel.seek_beat(&mut self.song, 1),
                    'a' => self.sel.seek_beat(&mut self.song, -1),
                    's' => self.sel.seek_string(&self.song, 1),
                    'w' => self.sel.seek_string(&self.song, -1),
                    _ => {}
                }
            }
            return;
        }
        if !cmd.starts_with(':') {
            match cmd.as_str() {
                "k" | "c" | "x" | "n" | "" => {}
                "z" => {
                    let res = self.undo();
                    self.set_typing_res(res);
                }
                "Z" => {
                    let res = self.redo();
                    self.set_typing_res(res);
                }
                "v" => self.push_paste_once(true),
                "V" => self.push_paste_once(false),
                "q" => self.should_close = true,
                "kb" => {
                    let res = self.push_action(Action::clear_beat(
                        self.sel.clone(),
                        self.sel.beat(&self.song).notes.clone(),
                    ));
                    self.set_typing_res(res);
                }
                "kn" | "xn" => {
                    let res = self.push_action(Action::set_note(
                        self.sel.clone(),
                        self.sel.beat(&self.song).copy_note(self.sel.string),
                        None,
                    ));
                    self.set_typing_res(res);
                }
                "xb" => {
                    let res = if let Some(count) = self.typing.parse_count() {
                        if let Some(b) = self.sel.beats_slice(&self.song, count) {
                            self.push_action(Action::delete_beats(self.sel.clone(), b.to_owned()))
                        } else {
                            Err(Error::InvalidOp("Tried to delete out of bounds".into()))
                        }
                    } else {
                        self.push_action(Action::delete_beat(
                            self.sel.clone(),
                            self.sel.beat(&self.song).clone(),
                        ))
                    };
                    self.set_typing_res(res);
                }
                "i" => {
                    let beat = self.sel.beat(&self.song).copy_duration();
                    let res = if let Some(count) = self.typing.parse_count() {
                        self.push_action(Action::paste_beats(
                            self.sel.clone(),
                            None,
                            vec![beat.clone(); count],
                        ))
                    } else {
                        self.push_action(Action::paste_beat(self.sel.clone(), None, beat))
                    };
                    self.set_typing_res(res);
                }
                _ => self.typing.clear(),
            }
        }
    }

    fn confirm_typing(&mut self) {
        if self.typing.cmd.starts_with(':') {
            let cmd = self.typing.cmd.get(1..).unwrap().to_owned();
            let res = self.proc_t_command(cmd);
            self.set_typing_res(res);
        } else if self.typing.cmd.starts_with('n') {
            let s = self.typing.cmd.get(1..).unwrap().to_owned();
            self.apply_note(&s);
            self.typing.clear();
        } else if self.typing.cmd.starts_with('l') {
            let s = self.typing.cmd.get(1..).unwrap();
            let res = match Duration::parse(s) {
                Ok(dur) => self.push_action(Action::set_duration(
                    self.sel.clone(),
                    self.sel.beat(&self.song).dur,
                    dur,
                )),
                Err(e) => Err(e),
            };
            self.set_typing_res(res);
            self.typing.clear();
        } else {
            self.set_typing_res(Ok(""));
        }
        self.typing.clear();
    }

    // Raw event processors

    fn key_press(&mut self, key: KeyCode, modi: KeyModifiers) {
        if self.typing.is_recieving() {
            if let KeyCode::Char(c) = key {
                self.typing.send_char(c);
                self.check_typing();
                return;
            }
            if let KeyCode::Backspace = key {
                self.typing.backspace();
                self.check_typing();
                return;
            }
        }
        match key {
            // Combo
            KeyCode::Esc => {
                if self.typing.is_recieving() {
                    self.typing.clear();
                } else {
                    self.should_close = true;
                }
            }
            KeyCode::Enter => self.confirm_typing(),
            // Cursor movement and scroll
            KeyCode::Right => {
                if modi.contains(KeyModifiers::SHIFT) {
                    self.sel.seek_scroll(&self.song, 5)
                } else {
                    self.sel.seek_scroll(&self.song, 1)
                }
                self.sel.cursor_to_scroll(self.s_bwidth);
            }
            KeyCode::Left => {
                if modi.contains(KeyModifiers::SHIFT) {
                    self.sel.seek_scroll(&self.song, -5)
                } else {
                    self.sel.seek_scroll(&self.song, -1)
                }
                self.sel.cursor_to_scroll(self.s_bwidth);
            }
            KeyCode::Char('d') => {
                self.sel.seek_beat(&mut self.song, 1);
                self.sel.scroll_to_cursor(self.s_bwidth);
            }
            KeyCode::Char('a') => {
                self.sel.seek_beat(&mut self.song, -1);
                self.sel.scroll_to_cursor(self.s_bwidth);
            }
            KeyCode::Char('D') => {
                self.sel.seek_beat(&mut self.song, 5);
                self.sel.scroll_to_cursor(self.s_bwidth);
            }
            KeyCode::Char('A') => {
                self.sel.seek_beat(&mut self.song, -5);
                self.sel.scroll_to_cursor(self.s_bwidth);
            }
            KeyCode::Char('s') => self.sel.seek_string(&mut self.song, 1),
            KeyCode::Char('w') => self.sel.seek_string(&mut self.song, -1),
            // Start combo
            KeyCode::Char(c) => {
                self.typing.send_char(c);
                self.check_typing();
            }
            _ => {}
        }
    }

    fn proc_event(&mut self, win: &mut window::Window) -> Result<bool> {
        match win.get_event() {
            Ok(e) => match e {
                event::Event::Key(e) => match e {
                    event::KeyEvent {
                        code, modifiers, ..
                    } => {
                        self.key_press(code, modifiers);
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
