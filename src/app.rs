use crate::{
    error::{Error, Result},
    window::{self, Color},
};
use crossterm::event;
use std::rc;

struct Track {
    string_count: u16,
}

struct Song {
    tracks: Vec<rc::Rc<Track>>,
}

struct Selection {
    beat: usize,
    string: u16,
}

pub struct App {
    win: window::Window,
    should_close: bool,
    song: Song,
    track: rc::Rc<Track>,
    sel: Selection,
}

impl App {
    pub fn new() -> Result<Self> {
        let song = Song {
            tracks: vec![rc::Rc::new(Track { string_count: 6 })],
        };
        Ok(Self {
            win: window::Window::new()?,
            should_close: false,
            track: song.tracks[0].clone(),
            song,
            sel: Selection { beat: 3, string: 1 },
        })
    }

    fn draw_durations(&mut self, max_count: u16) -> Result<()> {
        self.win.moveto(0, 0)?;
        for _ in 0..max_count {
            self.win.print("+ - ")?;
        }
        Ok(())
    }

    fn draw_string(&mut self, string: u16, max_count: usize) -> Result<()> {
        let sel_string = self.sel.string == string;
        self.win.moveto(0, string + 1)?;
        for i in 0..max_count {
            if self.sel.beat == i {
                self.win.print("|")?.print_color(
                    " 0 ",
                    if sel_string {
                        Color::WhiteBG
                    } else {
                        Color::GreyBG
                    },
                )?;
            } else {
                self.win.print("| 0 ")?;
            }
        }
        Ok(())
    }

    pub fn draw(&mut self, (w, h): (u16, u16)) -> Result<()> {
        self.win.clear()?.moveto(0, 0)?;
        self.draw_durations(w / 4)?;
        for i in 0..self.track.string_count {
            self.draw_string(i, (w / 4) as usize)?;
        }
        self.win.moveto(w - 1, h - 1)?.update()
    }

    pub fn proc_event(&mut self) -> Result<()> {
        match self.win.get_event() {
            Ok(e) => {
                match e {
                    event::Event::Key(e) => match e {
                        event::KeyEvent { code, .. } => match code {
                            event::KeyCode::Char('q') => self.should_close = true,
                            event::KeyCode::Char('a') => self.sel.beat -= 1,
                            event::KeyCode::Char('d') => self.sel.beat += 1,
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
        while !self.should_close {
            self.proc_event()?;
            self.draw(crossterm::terminal::size().unwrap())?;
        }
        self.win.clear()?.update()
    }
}
