use std::env::var;

lazy_static! {
    pub static ref TASK_FILE: String = format!("{}/.task.md", var("HOME").unwrap());
}