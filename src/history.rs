use crate::{
    cursor::Cursor,
    dur::Duration,
    song::{Beat, Note},
};
use std::collections::VecDeque;

pub enum Action {
    SetDuration {
        cur: Cursor,
        old: Duration,
        new: Duration,
    },
    SetNote {
        cur: Cursor,
        old: Option<Note>,
        new: Option<Note>,
    },
    ClearBeat {
        cur: Cursor,
        old: Vec<(u16, Note)>,
    },
    ClearBeats {
        cur: Cursor,
        old: Vec<Beat>,
    },
    DeleteBeat {
        cur: Cursor,
        old: Beat,
    },
    DeleteBeats {
        cur: Cursor,
        old: Vec<Beat>,
    },
    PasteNote {
        cur: Cursor,
        old: Option<Note>,
        buf: Note,
    },
    PasteBeat {
        cur: Cursor,
        old: Option<Beat>,
        buf: Beat,
    },
    PasteBeats {
        cur: Cursor,
        old: Option<Beat>,
        buf: Vec<Beat>,
    },
}

impl Action {
    pub fn set_duration(cur: Cursor, old: Duration, new: Duration) -> Self {
        Self::SetDuration { cur, old, new }
    }

    pub fn set_note(cur: Cursor, old: Option<Note>, new: Option<Note>) -> Self {
        Self::SetNote { cur, old, new }
    }

    pub fn clear_beat(cur: Cursor, old: Vec<(u16, Note)>) -> Self {
        Self::ClearBeat { cur, old }
    }

    pub fn clear_beats(cur: Cursor, old: Vec<Beat>) -> Self {
        Self::ClearBeats { cur, old }
    }

    pub fn delete_beat(cur: Cursor, old: Beat) -> Self {
        Self::DeleteBeat { cur, old }
    }

    pub fn delete_beats(cur: Cursor, old: Vec<Beat>) -> Self {
        Self::DeleteBeats { cur, old }
    }

    pub fn paste_note(cur: Cursor, old: Option<Note>, buf: Note) -> Self {
        Self::PasteNote { cur, old, buf }
    }

    pub fn paste_beat(cur: Cursor, old: Option<Beat>, buf: Beat) -> Self {
        Self::PasteBeat { cur, old, buf }
    }

    pub fn paste_beats(cur: Cursor, old: Option<Beat>, buf: Vec<Beat>) -> Self {
        Self::PasteBeats { cur, old, buf }
    }
}

pub struct History {
    size: usize,
    history: VecDeque<std::rc::Rc<Action>>,
    future: usize,
}

impl History {
    pub fn new(size: usize) -> Self {
        Self {
            size,
            history: VecDeque::with_capacity(size),
            future: 0,
        }
    }

    fn del_old(&mut self) {
        self.history.pop_back();
    }

    fn del_future(&mut self) {
        for _ in 0..self.future {
            self.history.pop_front();
        }
        self.future = 0;
    }

    pub fn redo(&mut self) -> Option<std::rc::Rc<Action>> {
        self.future = self.future.checked_sub(1)?;
        Some(self.history.get(self.future)?.to_owned())
    }

    pub fn undo(&mut self) -> Option<std::rc::Rc<Action>> {
        let e = self.history.get(self.future)?;
        self.future += 1;
        Some(e.to_owned())
    }

    pub fn push(&mut self, entry: std::rc::Rc<Action>) {
        self.del_future();
        if self.history.len() == self.size {
            self.del_old();
        }
        self.history.push_front(entry);
    }
}
