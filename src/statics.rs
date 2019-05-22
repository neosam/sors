use std::env::var;

lazy_static! {
    pub static ref TASK_FILE: String = format!("{}/.task.md", var("HOME").unwrap());
    pub static ref HISTORY_FILE: String = format!("{}/.taskhistory", var("HOME").unwrap());
}