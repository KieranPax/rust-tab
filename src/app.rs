use crate::{
    error::{Error, Result},
    window::{self, Color},
};
use crossterm::event;
use std::{cell, rc};

struct Note {}

#[derive(Clone, Copy)]
enum Duration {
    Whole,
    Half,
    Quarter,
    Eighth,
    Sixteenth,
    ThirtyTwoth,
}

struct Beat {
    dur: Duration,
}

impl Beat {
    fn copy_duration(&self) -> Self {
        Self { dur: self.dur }
    }
}

struct Track {
    string_count: u16,
    beats: Vec<Beat>,
}

struct Song {
    tracks: Vec<rc::Rc<cell::RefCell<Track>>>,
}

pub struct App {
    should_close: bool,
    song: Song,
    track: rc::Rc<cell::RefCell<Track>>,
    sel_beat: u32,
    sel_string: u16,
}

impl App {
    pub fn new() -> Result<Self> {
        let song = Song {
            tracks: vec![rc::Rc::new(cell::RefCell::new(Track {
                string_count: 6,
                beats: vec![
                    Beat {
                        dur: Duration::Quarter,
                    },
                    Beat {
                        dur: Duration::Quarter,
                    },
                    Beat {
                        dur: Duration::Quarter,
                    },
                    Beat {
                        dur: Duration::Quarter,
                    },
                ],
            }))],
        };
        Ok(Self {
            should_close: false,
            track: song.tracks[0].clone(),
            song,
            sel_beat: 0,
            sel_string: 0,
        })
    }

    fn draw_durations(&self, win: &mut window::Window, max_count: u16) -> Result<()> {
        win.moveto(0, 0)?;
        for _ in 0..max_count {
            win.print("+ - ")?;
        }
        Ok(())
    }

    fn draw_string(&self, win: &mut window::Window, string: u16, max_count: u32) -> Result<()> {
        let sel_string = self.sel_string == string;
        win.moveto(0, string + 1)?;
        for i in 0..max_count {
            if self.sel_beat == i {
                win.print("|")?.print_color(
                    " 0 ",
                    if sel_string {
                        Color::WhiteBG
                    } else {
                        Color::GreyBG
                    },
                )?;
            } else {
                win.print("| 0 ")?;
            }
        }
        Ok(())
    }

    fn draw(&self, win: &mut window::Window, (w, _h): (u16, u16)) -> Result<()> {
        win.clear()?.moveto(0, 0)?;
        self.draw_durations(win, w / 4)?;
        for i in 0..self.track.borrow().string_count {
            self.draw_string(win, i, (w / 4) as u32)?;
        }
        win.moveto(0, self.track.borrow().string_count + 2)?
            .update()
    }

    fn seek_string(&mut self, dire: i16) {
        let new = self.sel_string as i16 + dire;
        self.sel_string = new.clamp(0, self.track.borrow().string_count as i16 - 1) as u16;
    }

    fn seek_beat(&mut self, dire: i32) {
        let new = self.sel_beat as i32 + dire;
        self.sel_beat = new.max(0) as u32;
        let beats = &mut self.track.borrow_mut().beats;
        while self.sel_beat > beats.len() as u32 {
            beats.push(beats.last().unwrap().copy_duration());
        }
    }

    fn proc_event(&mut self, win: &mut window::Window) -> Result<()> {
        match win.get_event() {
            Ok(e) => {
                match e {
                    event::Event::Key(e) => match e {
                        event::KeyEvent { code, .. } => match code {
                            event::KeyCode::Char('q') => self.should_close = true,
                            event::KeyCode::Char('a') => self.seek_beat(-1),
                            event::KeyCode::Char('d') => self.seek_beat(1),
                            _ => {}
                        },
                    },
                    _ => {}
                }
                Ok(())
            }
            Err(Error::NoEvent) => Ok(()),
            Err(e) => Err(e),
        }
    }

    pub fn run(mut self) -> Result<()> {
        let mut win = window::Window::new()?;
        while !self.should_close {
            self.proc_event(&mut win)?;
            self.draw(&mut win, crossterm::terminal::size().unwrap())?;
        }
        win.clear()?.update()
    }
}
