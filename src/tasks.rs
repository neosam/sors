use uuid::Uuid;
use serde::{Serialize, Deserialize};
use std::rc::Rc;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum Progress {
    Todo, Work, Done
}
impl Progress {
    pub fn done(self) -> bool {
        match self {
            Progress::Todo => false,
            Progress::Work => false,
            Progress::Done => true
        }
    }
}

impl ToString for Progress {
    fn to_string(&self) -> String {
        match self {
            Progress::Todo => "TODO".to_string(),
            Progress::Work => "WORK".to_string(),
            Progress::Done => "DONE".to_string()
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Task {
    pub id: Uuid,
    pub title: String,
    pub body: String,
    pub children: Vec<Uuid>,
    pub progress: Option<Progress>
}

impl Default for Task {
    fn default() -> Self {
        Task::new()
    }
}

impl Task {
    pub fn new() -> Task {
        let root_id = Uuid::new_v4();
        Task {
            id: root_id,
            title: String::new(),
            body: String::new(),
            children: Vec::new(),
            progress: None
        }
    }
}

pub trait TaskMod {
    fn set_title(&mut self, title: impl ToString) -> &mut Self;
    fn set_body(&mut self, body: impl ToString) -> &mut Self;
    fn set_children(&mut self, children: Vec<Uuid>) -> &mut Self;
    fn add_child(&mut self, child: Uuid) -> &mut Self;
    fn insert_child(&mut self, child: Uuid, index: usize) -> &mut Self;
    fn remove_child(&mut self, child: &Uuid) -> &mut Self;
    fn set_progress(&mut self, progress: Progress) -> &mut Self;
}
impl TaskMod for Rc<Task> {
    fn set_title(&mut self, title: impl ToString) -> &mut Self {
        Rc::make_mut(self).title = title.to_string();
        self
    }
    fn set_body(&mut self, body: impl ToString) -> &mut Self {
        Rc::make_mut(self).body = body.to_string();
        self
    }
    fn set_children(&mut self, children: Vec<Uuid>) -> &mut Self {
        Rc::make_mut(self).children = children;
        self
    }
    fn add_child(&mut self, child: Uuid) -> &mut Self {
        let mut children = self.children.clone();
        children.push(child);
        self.set_children(children);
        self
    }
    fn insert_child(&mut self, child: Uuid, index: usize) -> &mut Self {
        let mut children = self.children.clone();
        children.insert(index, child);
        self.set_children(children);
        self
    }
    fn remove_child(&mut self, child_id: &Uuid) -> &mut Self {
        let children = self.children.iter().filter_map(|child| 
            if child == child_id {
                None
            } else {
                Some(*child)
            }
        ).collect();
        self.set_children(children);
        self
    }
    fn set_progress(&mut self, progress: Progress) -> &mut Self {
        Rc::make_mut(self).progress = Some(progress);
        self
    }
}