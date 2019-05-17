use uuid::Uuid;
use std::rc::Rc;
use chrono::prelude::*;
use serde::{Serialize, Deserialize};


#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Clock {
    pub id: Uuid,
    pub start: DateTime<Local>,
    pub end: Option<DateTime<Local>>,
    pub comment: Option<String>,
    pub task_id: Option<Uuid>
}

impl std::cmp::PartialEq for Clock {
    fn eq(&self, o: &Self) -> bool {
        self.start == o.start
    }
}
impl std::cmp::Eq for Clock {}
impl std::cmp::PartialOrd for Clock {
    fn partial_cmp(&self, o: &Self) -> Option<std::cmp::Ordering> {
        self.start.partial_cmp(&o.start)
    }
}
impl std::cmp::Ord for Clock {
    fn cmp(&self, o: &Self) -> std::cmp::Ordering {
        self.start.cmp(&o.start)
    }
}

impl Clock {
    pub fn duration(&self) -> chrono::Duration {
        self.end.unwrap_or_else(|| Local::now()) - self.start
    }
}

pub trait ClockMod {
    fn set_start(&mut self, start: DateTime<Local>) -> &mut Self;
    fn set_end(&mut self, end: DateTime<Local>) -> &mut Self;
    fn set_comment(&mut self, comment: String) -> &mut Self;
    fn set_task_id(&mut self, task_id: Uuid) -> &mut Self;
}

impl ClockMod for Rc<Clock> {
    fn set_start(&mut self, start: DateTime<Local>) -> &mut Self {
        Rc::make_mut(self).start = start;
        self
    }
    fn set_end(&mut self, end: DateTime<Local>) -> &mut Self {
        Rc::make_mut(self).end = Some(end);
        self
    }
    fn set_comment(&mut self, comment: String) -> &mut Self {
        Rc::make_mut(self).comment = Some(comment);
        self
    }
    fn set_task_id(&mut self, task_id: Uuid) -> &mut Self {
        Rc::make_mut(self).task_id = Some(task_id);
        self
    }
}