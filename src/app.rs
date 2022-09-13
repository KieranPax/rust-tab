use crate::{
    buffer::Buffer,
    cursor::Cursor,
    dur::Duration,
    error::{Error, Result},
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

enum InpMode {
    None,
    Measure,
    Beat,
    Note,
    Edit,
    Duration,
}

struct InpCtrl {
    mode: InpMode,
    arg: String,
}

impl InpCtrl {
    fn new() -> Self {
        Self {
            mode: InpMode::None,
            arg: String::new(),
        }
    }

    fn clear(&mut self) {
        self.mode = InpMode::None;
        self.arg.clear();
    }

    fn is_none(&self) -> bool {
        matches!(self.mode, InpMode::None)
    }

    fn display(&self) -> String {
        match &self.mode {
            InpMode::None => self.arg.clone(),
            InpMode::Measure => format!("m:{}", self.arg),
            InpMode::Beat => format!("b:{}", self.arg),
            InpMode::Note => format!("n:{}", self.arg),
            InpMode::Edit => format!("e:{}", self.arg),
            InpMode::Duration => format!("d:{}", self.arg),
        }
    }

    fn char_valid(&self, ch: char) -> bool {
        match self.mode {
            InpMode::Duration => ch.is_ascii_digit() || ch == ':' || ch == '/',
            InpMode::Edit => ch.is_ascii_digit() || ch == 'x',
            InpMode::Note | InpMode::Beat | InpMode::Measure => ch.is_ascii_digit(),
            InpMode::None => false,
        }
    }
}

pub struct App {
    args: Args,
    should_close: bool,
    song_path: Option<String>,
    song: Song,
    sel: Cursor,
    input: InpCtrl,
    command_res: String,
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
            input: InpCtrl::new(),
            command_res: String::new(),
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

    fn new_action(&mut self, action: Action) {
        let res = self.push_action(action);
        self.set_command_res(res);
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
        if self.input.is_none() {
            format!("{} | buffer : {:?}", self.command_res, self.copy_buf)
        } else {
            format!(">{}< | buffer : {:?}", self.input.display(), self.copy_buf)
        }
    }

    fn set_command_res<T>(&mut self, res: Result<T>)
    where
        T: Into<String>,
    {
        if let Err(e) = res {
            self.command_res = format!("{e}");
        } else {
            self.command_res = res.unwrap().into();
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

    // Input handling

    fn key_press(&mut self, key: KeyCode, modi: KeyModifiers) {
        match key {
            KeyCode::Esc => self.should_close = true,
            KeyCode::Char('d') => self.sel.seek_beat(&mut self.song, 1, self.s_bwidth),
            KeyCode::Char('a') => self.sel.seek_beat(&mut self.song, -1, self.s_bwidth),
            KeyCode::Char('s') => self.sel.seek_string(&self.song, 1),
            KeyCode::Char('w') => self.sel.seek_string(&self.song, -1),
            KeyCode::Right => self.sel.seek_scroll(&mut self.song, 1, self.s_bwidth),
            KeyCode::Left => self.sel.seek_scroll(&mut self.song, -1, self.s_bwidth),

            KeyCode::Char('z') => {
                let res = self.undo();
                self.set_command_res(res);
            }
            KeyCode::Char('y') => {
                let res = self.redo();
                self.set_command_res(res);
            }

            KeyCode::Char('l') => self.input.mode = InpMode::Duration,
            KeyCode::Char('e') => self.input.mode = InpMode::Edit,
            KeyCode::Char('n') => self.input.mode = InpMode::Note,
            KeyCode::Char('b') => self.input.mode = InpMode::Beat,
            KeyCode::Char('m') => self.input.mode = InpMode::Measure,
            _ => {}
        }
    }

    fn input_duration(&mut self) {
        if !self.input.arg.is_empty() {
            match Duration::parse(&self.input.arg) {
                Ok(dur) => self.new_action(Action::set_duration(
                    self.sel.clone(),
                    self.sel.beat(&self.song).dur.clone(),
                    dur,
                )),
                Err(e) => self.set_command_res::<&str>(Err(e)),
            };
            self.input.clear();
        }
    }

    fn input_edit(&mut self) {
        match Note::parse(&self.input.arg) {
            Ok(note) => self.new_action(Action::set_note(
                self.sel.clone(),
                self.sel.beat(&self.song).copy_note(self.sel.string),
                Some(note),
            )),
            Err(e) => self.set_command_res::<&str>(Err(e)),
        }
        self.input.clear();
    }

    fn key_input(&mut self, key: KeyCode) {
        if matches!(key, KeyCode::Esc) {
            return self.input.clear();
        }
        if let KeyCode::Char(ch) = key {
            if self.input.char_valid(ch) {
                return self.input.arg.push(ch);
            }
        }
        match self.input.mode {
            InpMode::Duration => match key {
                KeyCode::Enter => self.input_duration(),
                KeyCode::Char('l') => {
                    self.input_duration();
                    self.sel.seek_beat(&mut self.song, 1, self.s_bwidth);
                    self.input.mode = InpMode::Duration;
                }
                _ => {}
            },
            InpMode::Edit => match key {
                KeyCode::Enter => self.input_edit(),
                KeyCode::Char('e') => {
                    self.input_edit();
                    self.sel.seek_beat(&mut self.song, 1, self.s_bwidth);
                    self.input.mode = InpMode::Edit;
                }
                _ => {}
            },
            InpMode::Note => match key {
                KeyCode::Char('c') => self.copy_buf = self.sel.copy_note(&mut self.song),
                KeyCode::Char('k') | KeyCode::Char('x') => self.new_action(Action::set_note(
                    self.sel.clone(),
                    self.sel.clone_note(&self.song),
                    None,
                )),
                _ => {}
            },
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
                        if self.input.is_none() {
                            self.key_press(code, modifiers);
                        } else {
                            self.key_input(code);
                        }
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
