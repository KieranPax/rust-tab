use crate::{cursor::Cursor, dur::Duration};
use std::collections::VecDeque;

pub struct ActionData<T0, T1> {
    pub cursor: Cursor,
    pub old: T0,
    pub new: T1,
}

pub enum Action {
    SetDuration(ActionData<Duration, Duration>),
    SetNote(ActionData<Option<u32>, u32>),
    DelNote(ActionData<u32, ()>),
}

impl Action {
    pub fn set_duration(cursor: Cursor, old: Duration, new: Duration) -> Self {
        Self::SetDuration(ActionData { cursor, old, new })
    }

    pub fn set_note(cursor: Cursor, old: Option<u32>, new: u32) -> Self {
        Self::SetNote(ActionData { cursor, old, new })
    }

    pub fn del_note(cursor: Cursor, old: u32) -> Self {
        Self::DelNote(ActionData { cursor, old, new: () })
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
