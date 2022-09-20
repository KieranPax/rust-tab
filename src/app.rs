use crate::{
    args,
    buffer::Buffer,
    cursor::Cursor,
    draw::Lane,
    dur::Duration,
    error::{Error, Result},
    history::{Action, History},
    song::{Note, Song},
    window,
};
use crossterm::event::{self, KeyCode, KeyModifiers};

enum InpMode {
    None,
    Measure,
    Beat,
    Note,
    Edit,
    Duration,
    Command,
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

    fn push(&mut self, ch: char) {
        self.arg.push(ch);
    }

    fn clear(&mut self) {
        self.mode = InpMode::None;
        self.arg.clear();
    }

    fn backspace(&mut self) {
        if self.arg.is_empty() {
            self.mode = InpMode::None;
        } else {
            self.arg.pop();
        }
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
            InpMode::Command => format!(":{}", self.arg),
        }
    }

    fn char_valid(&self, ch: &char) -> bool {
        match self.mode {
            InpMode::Duration => ch.is_ascii_digit() || ch == &':' || ch == &'/',
            InpMode::Edit => ch.is_ascii_digit() || ch == &'x',
            InpMode::Note | InpMode::Beat | InpMode::Measure => ch.is_ascii_digit(),
            InpMode::Command => ch.is_alphabetic() || ch == &'_' || ch == &' ',
            InpMode::None => false,
        }
    }

    fn arg_clear(&mut self) -> String {
        let temp = self.arg.clone();
        self.clear();
        temp
    }

    fn parse_arg_clear<T: std::str::FromStr<Err = Error>>(&mut self) -> Result<T> {
        let temp = self.arg.parse();
        self.clear();
        temp
    }

    fn parse_arg_opt_clear<T: std::str::FromStr>(&mut self) -> Option<T> {
        let temp = self.arg.parse().ok();
        self.clear();
        temp
    }
}

pub struct App {
    args: args::Args,
    should_close: bool,
    song_path: Option<String>,
    song: Song,
    lanes: Vec<Lane>,
    curr_lane: usize,
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
            args: clap::Parser::parse(),
            should_close: false,
            song_path: None,
            song: Song::new(),
            lanes: Vec::new(),
            curr_lane: 0,
            input: InpCtrl::new(),
            command_res: String::new(),
            copy_buf: Buffer::Empty,
            s_bwidth: 4,
            s_height: 4,
            history: History::new(32),
        })
    }

    pub fn cursor(&self) -> &Cursor {
        &self.lanes[self.curr_lane].cur
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
            Action::ClearBeats { cur, old } => {
                cur.clear_beats(&mut self.song, old.len());
                Ok("Clear beats".into())
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
            Action::ClearBeats { cur, old } => {
                cur.replace_beats(&mut self.song, old.clone());
                Ok("Undo clear beats".into())
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

    fn do_save_file(&mut self, inp: Option<String>) {
        let res = if let Some(path) = inp {
            self.save_file(path)
        } else {
            if let Some(path) = self.song_path.clone() {
                self.save_file(path)
            } else {
                Err(Error::FileError("No default file to save to".into()))
            }
        };
        self.set_command_res(res);
    }

    fn load_file(&mut self, path: String) -> Result<String> {
        if let Ok(data) = std::fs::read_to_string(&path) {
            self.song = serde_json::from_str(data.as_str()).unwrap();
            for track in self.song.tracks.iter_mut() {
                track.update_measures();
            }
            Ok(format!("Loaded {path}"))
        } else {
            Err(Error::InvalidOp(format!("Cannot read file '{path}'")))
        }
    }

    fn do_load_file(&mut self, inp: Option<&&str>) {
        let res = if let Some(path) = inp {
            self.load_file(path.to_string())
        } else {
            if let Some(path) = self.song_path.clone() {
                self.load_file(path)
            } else {
                Err(Error::FileError("No default file to save to".into()))
            }
        };
        self.set_command_res(res);
    }

    // Draw functions

    fn reset_sdim(&mut self, (w, h): (u16, u16)) {
        self.s_bwidth = ((w - 4) / 4) as usize;
        self.s_height = h;
    }

    fn gen_status_msg(&self) -> String {
        if self.input.is_none() {
            format!("{} | buffer : {:?}", self.command_res, self.copy_buf)
        } else {
            format!(">{}< | buffer : {:?}", self.input.display(), self.copy_buf)
        }
    }

    fn set_command_res<T: Into<String>>(&mut self, res: Result<T>) {
        if let Err(e) = res {
            self.command_res = format!("{e}");
        } else {
            self.command_res = res.unwrap().into();
        }
    }

    fn set_command_err(&mut self, err: Error) {
        self.command_res = format!("{err}");
    }

    fn draw(&self, win: &mut window::Window) -> Result<()> {
        let t0 = std::time::Instant::now();
        win.moveto(0, 0)?;
        for (i, lane) in self.lanes.iter().enumerate() {
            lane.draw(win, self.s_bwidth, &self.song, i == self.curr_lane)?;
        }
        win.print(self.gen_status_msg())?;
        let dur = std::time::Instant::now().duration_since(t0).as_secs_f32() * 1000.0;
        if self.args.draw_timer {
            win.print(format!("     -> ({dur:.2}ms)"))?;
        }
        win.clear_eoline()?.update()?;
        Ok(())
    }

    // Actions

    fn do_set_duration(&mut self, dur: Duration) {
        self.new_action(Action::set_duration(
            self.cursor().clone(),
            self.cursor().beat(&self.song).dur.clone(),
            dur,
        ));
    }

    fn do_set_note(&mut self, note: Option<Note>) {
        self.new_action(Action::set_note(
            self.cursor().clone(),
            self.cursor()
                .beat(&self.song)
                .copy_note(self.cursor().string),
            note,
        ));
    }

    fn do_copy_note(&mut self) {
        self.copy_buf = self.cursor().copy_note(&self.song);
        if matches!(self.copy_buf, Buffer::Note(_)) {
            self.set_command_res(Ok("Copied Note"));
        }
    }

    fn do_copy_beat(&mut self) {
        self.copy_buf = self.cursor().copy_beat(&self.song);
        if matches!(self.copy_buf, Buffer::Beat(_)) {
            self.set_command_res(Ok("Copied Beat"));
        }
    }

    fn do_copy_beats(&mut self, count: usize) {
        self.copy_buf = self.cursor().copy_beats(&self.song, count);
        if let Buffer::Beats(b) = &self.copy_buf {
            let msg = format!("Copied {} beats", b.len());
            self.set_command_res(Ok(msg));
        }
    }

    fn do_delete_beats(&mut self, count: usize) {
        if let Some(b) = self.cursor().clone_beats_slice(&self.song, count) {
            self.new_action(Action::delete_beats(self.cursor().clone(), b))
        } else {
            self.set_command_err(Error::InvalidOp("Tried to delete out of bounds".into()));
        }
    }

    fn do_delete_beat(&mut self) {
        self.new_action(Action::delete_beat(
            self.cursor().clone(),
            self.cursor().clone_beat(&self.song),
        ));
    }

    fn do_clear_beats(&mut self, count: usize) {
        if let Some(b) = self.cursor().clone_beats_slice(&self.song, count) {
            self.new_action(Action::clear_beats(self.cursor().clone(), b))
        } else {
            self.set_command_err(Error::InvalidOp("Tried to delete out of bounds".into()));
        }
    }

    fn do_clear_beat(&mut self) {
        self.new_action(Action::clear_beat(
            self.cursor().clone(),
            self.cursor().clone_chord(&self.song),
        ));
    }

    fn do_paste(&mut self, in_place: bool) {
        match self.copy_buf.clone() {
            Buffer::Note(note) => self.new_action(Action::paste_note(
                self.cursor().clone(),
                self.cursor().clone_note(&self.song),
                note,
            )),
            Buffer::Beat(beat) => self.new_action(Action::paste_beat(
                self.cursor().clone(),
                if in_place {
                    Some(self.cursor().clone_beat(&self.song))
                } else {
                    None
                },
                beat,
            )),
            Buffer::Beats(beats) => self.new_action(Action::paste_beats(
                self.cursor().clone(),
                if in_place {
                    Some(self.cursor().clone_beat(&self.song))
                } else {
                    None
                },
                beats,
            )),
            _ => {}
        }
    }

    // Cursor functions

    fn sync_cursors(&mut self) {
        let dur = self.lanes[self.curr_lane].cur.calc_duration(&self.song);
        for (i, lane) in self.lanes.iter_mut().enumerate() {
            if i != self.curr_lane {
                lane.cur.transfer_seek(dur, &self.song, self.s_bwidth);
            }
        }
    }

    fn cur_seek_beat(&mut self, dire: isize) {
        self.lanes[self.curr_lane]
            .cur
            .seek_beat(&mut self.song, dire, self.s_bwidth);
        self.sync_cursors();
    }

    fn cur_seek_next_measure(&mut self) {
        self.lanes[self.curr_lane]
            .cur
            .seek_next_measure(&self.song, self.s_bwidth);
        self.sync_cursors();
    }

    fn cur_seek_prev_measure(&mut self) {
        self.lanes[self.curr_lane]
            .cur
            .seek_prev_measure(&self.song, self.s_bwidth);
        self.sync_cursors();
    }

    fn cur_seek_end(&mut self) {
        self.lanes[self.curr_lane]
            .cur
            .seek_end(&self.song, self.s_bwidth);
        self.sync_cursors();
    }

    fn cur_seek_start(&mut self) {
        self.lanes[self.curr_lane].cur.seek_start();
        self.sync_cursors();
    }

    fn cur_seek_scroll(&mut self, dire: isize) {
        self.lanes[self.curr_lane]
            .cur
            .seek_scroll(&mut self.song, dire, self.s_bwidth);
        self.sync_cursors();
    }

    fn cur_seek_string(&mut self, dire: i16) {
        self.lanes[self.curr_lane]
            .cur
            .seek_string(&mut self.song, dire);
    }

    fn cur_next_lane(&mut self) {
        self.curr_lane += 1;
        if self.curr_lane == self.lanes.len() {
            self.curr_lane = 0;
        }
    }

    fn cur_prev_lane(&mut self) {
        if self.curr_lane == 0 {
            self.curr_lane = self.lanes.len();
        }
        self.curr_lane -= 1;
    }

    // Input handling

    fn key_press(&mut self, key: KeyCode, modi: KeyModifiers) {
        let shift = modi.contains(KeyModifiers::SHIFT);
        match key {
            KeyCode::Esc => self.should_close = true,

            KeyCode::Char('D') => self.cur_seek_next_measure(),
            KeyCode::Char('A') => self.cur_seek_prev_measure(),
            KeyCode::Char('d') => self.cur_seek_beat(1),
            KeyCode::Char('a') => self.cur_seek_beat(-1),
            KeyCode::End => self.cur_seek_end(),
            KeyCode::Home => self.cur_seek_start(),

            KeyCode::Right if shift => self.cur_seek_scroll(5),
            KeyCode::Left if shift => self.cur_seek_scroll(-5),
            KeyCode::Right => self.cur_seek_scroll(1),
            KeyCode::Left => self.cur_seek_scroll(-1),
            KeyCode::Down => self.cur_next_lane(),
            KeyCode::Up => self.cur_prev_lane(),

            KeyCode::Char('s') => self.cur_seek_string(1),
            KeyCode::Char('w') => self.cur_seek_string(-1),
            KeyCode::Char('z') => {
                let res = self.undo();
                self.set_command_res(res);
            }
            KeyCode::Char('y') => {
                let res = self.redo();
                self.set_command_res(res);
            }

            KeyCode::Char('v') => self.do_paste(false),
            KeyCode::Char('V') => self.do_paste(false),
            KeyCode::Char('c') => {
                self.set_command_err(Error::InvalidOp("Specify copy type first".into()))
            }

            KeyCode::Char('l') => self.input.mode = InpMode::Duration,
            KeyCode::Char('e') => self.input.mode = InpMode::Edit,
            KeyCode::Char('n') => self.input.mode = InpMode::Note,
            KeyCode::Char('b') => self.input.mode = InpMode::Beat,
            KeyCode::Char('m') => self.input.mode = InpMode::Measure,
            KeyCode::Char(':') => self.input.mode = InpMode::Command,
            _ => {}
        }
    }

    fn input_duration(&mut self) {
        match self.input.parse_arg_clear() {
            Ok(dur) => self.do_set_duration(dur),
            Err(e) => self.set_command_err(e),
        };
    }

    fn input_edit(&mut self) {
        match self.input.parse_arg_clear() {
            Ok(note) => self.do_set_note(Some(note)),
            Err(e) => self.set_command_err(e),
        }
    }

    fn input_command(&mut self) {
        let arg = self.input.arg_clear();
        let cmd = if let Some((a, b)) = arg.split_once(' ') {
            (a, Some(b))
        } else {
            (arg.as_str(), None)
        };
        match cmd {
            ("save", Some(path)) => {
                let path = Some(path.to_owned());
                self.do_save_file(path);
            }
            ("save", None) => self.do_save_file(None),
            _ => {}
        }
    }

    fn key_input(&mut self, key: KeyCode) {
        match &key {
            KeyCode::Esc => self.input.clear(),
            KeyCode::Backspace => self.input.backspace(),
            KeyCode::Char(ch) if self.input.char_valid(ch) => self.input.push(ch.to_owned()),
            _ => match self.input.mode {
                InpMode::Duration => match key {
                    KeyCode::Enter => self.input_duration(),
                    KeyCode::Char('l') => {
                        self.input_duration();
                        self.cur_seek_beat(1);
                        self.input.mode = InpMode::Duration;
                    }
                    _ => {}
                },
                InpMode::Edit => match key {
                    KeyCode::Enter => self.input_edit(),
                    KeyCode::Char('e') => {
                        self.input_edit();
                        self.cur_seek_beat(1);
                        self.input.mode = InpMode::Edit;
                    }
                    _ => {}
                },
                InpMode::Note => match key {
                    KeyCode::Char('c') => {
                        self.do_copy_note();
                        self.input.clear();
                    }
                    KeyCode::Char('k') | KeyCode::Char('x') => {
                        self.do_set_note(None);
                        self.input.clear();
                    }
                    _ => {}
                },
                InpMode::Beat => match key {
                    KeyCode::Char('c') => match self.input.parse_arg_opt_clear() {
                        Some(n) => self.do_copy_beats(n),
                        None => self.do_copy_beat(),
                    },
                    KeyCode::Char('x') => match self.input.parse_arg_opt_clear() {
                        Some(n) => self.do_delete_beats(n),
                        None => self.do_delete_beat(),
                    },
                    KeyCode::Char('k') => match self.input.parse_arg_opt_clear::<usize>() {
                        Some(n) => self.do_clear_beats(n),
                        None => self.do_clear_beat(),
                    },
                    _ => {}
                },
                InpMode::Command => match key {
                    KeyCode::Enter => self.input_command(),
                    _ => {}
                },
                _ => {}
            },
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
        let _ = self.do_load_file(None);

        let mut win = window::Window::new()?;
        win.clear()?;
        self.reset_sdim(crossterm::terminal::size().unwrap());
        let mut do_redraw = true;
        self.lanes.push(Lane::new());
        self.lanes.push(Lane::new_t(1));
        while !self.should_close {
            if do_redraw {
                self.draw(&mut win)?;
            }
            do_redraw = self.proc_event(&mut win)?;
        }
        win.clear()?.update()
    }
}
