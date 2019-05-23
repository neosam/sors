use super::doc::*;
use uuid::Uuid;

#[derive(Debug, PartialEq, Eq)]
pub enum Autosave {
    ManualOnly,
    OnCommand
}

#[derive(Debug)]
pub struct State {
    pub doc: Doc,
    pub wt: Uuid,
    pub parents: Vec<Uuid>,
    pub path: String,
    pub autosave: Autosave
}

impl State {
    pub fn uuid_for_path(&self, path: &str) -> Option<Uuid> {
        let mut current_task = if path.starts_with("/") {
            Some(self.doc.root.clone())
        } else {
            Some(self.wt.clone())
        };
        let splitted_path = path.split("/");
        
        for part in splitted_path {
            if let Ok(i) = part.parse::<usize>() {
                if let Some(task) = current_task {
                    current_task = self.doc.task_child(&task, i - 1);
                } else {
                    current_task = None;
                }
            } else if let Ok(id) = part.parse::<Uuid>() {
                current_task = Some(id)
            } else if part == ".." {
                if let Some(task) = current_task {
                    current_task = self.doc.find_parent(&task);
                }
            } else if part == "" {
                // Empty - Do nothing
            } else {
                if let Some(task) = current_task {
                    current_task = self.doc.task_child_prefix(&task, part);
                }
            }
        }
        current_task
    }
}