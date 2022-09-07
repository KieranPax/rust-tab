use crate::{
    error::{Error, Result},
    map_io_err,
};
use crossterm::{
    event,
    style::{self, Stylize},
    terminal,
};

#[allow(unused)]
pub enum Color {
    Magenta,
    WhiteBG,
    GreyBG,
}

impl Color {
    pub fn stylize<'a>(&self, text: &'a str) -> style::StyledContent<&'a str> {
        match self {
            Self::Magenta => Stylize::magenta(text),
            Self::WhiteBG => Stylize::on_white(text).black(),
            Self::GreyBG => Stylize::on_dark_grey(text),
        }
    }
}

pub struct Window {
    stdout: std::io::Stdout,
}

impl Window {
    pub fn new() -> Result<Self> {
        let mut o = Self {
            stdout: std::io::stdout(),
        };
        o.queue(crossterm::cursor::Hide)?
            .queue(crossterm::terminal::SetTitle("Tab"))?
            .update()?;
        Ok(o)
    }

    pub fn moveto(&mut self, x: u16, y: u16) -> Result<&mut Self> {
        self.queue(crossterm::cursor::MoveTo(x, y))
    }

    pub fn print_color(&mut self, text: &str, color: Color) -> Result<&mut Self> {
        self.queue(style::PrintStyledContent(color.stylize(text)))
    }

    pub fn print(&mut self, text: &str) -> Result<&mut Self> {
        self.queue(style::Print(text))
    }

    pub fn clear(&mut self) -> Result<&mut Self> {
        self.queue(terminal::Clear(terminal::ClearType::All))
    }

    pub fn clear_line(&mut self) -> Result<&mut Self> {
        self.queue(terminal::Clear(terminal::ClearType::CurrentLine))
    }

    pub fn queue<C>(&mut self, command: C) -> Result<&mut Self>
    where
        C: crossterm::Command,
    {
        map_io_err!(crossterm::QueueableCommand::queue(
            &mut self.stdout,
            command
        ))?;
        Ok(self)
    }

    pub fn update(&mut self) -> Result<()> {
        map_io_err!(std::io::Write::flush(&mut self.stdout))
    }

    pub fn get_event(&mut self) -> Result<event::Event> {
        let poll = map_io_err!(event::poll(std::time::Duration::from_millis(100)))?;
        if poll {
            map_io_err!(event::read())
        } else {
            Err(Error::NoEvent)
        }
    }
}
