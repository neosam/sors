use crate::clockedit::*;
use crate::error::*;
use crate::doc::*;
use crate::helper::*;
use crate::cli::{Cli, CliCallbacks, CliStateCallback};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExitAction {
    Cancel,
    Apply
}

#[derive(Debug, Clone)]
pub struct ClockEditCli<'a> {
    pub clockedit: ClockEdit,
    pub apply_result: ExitAction,
    pub doc: &'a Doc,
}

pub struct ClockCallbacks;
impl<'a> CliStateCallback<ClockEditCli<'a>> for ClockCallbacks {}

impl<'a> ClockEditCli<'a> {
    pub fn apply_commands<C: CliCallbacks<ClockEditCli<'a>>>(terminal: &mut Cli<ClockEditCli<'a>, C>) {
        terminal.register_command("cancel", Box::new(|_, _, callbacks| {
            callbacks.exit();
            Ok(())
        }));
        terminal.register_command("start", Box::new(|state: &mut ClockEditCli, line: &str, _| {
            let mut splitted_line = line.split(" ");
            splitted_line.next();
            let i = if let Some(index) = splitted_line.next() {
                index.parse::<usize>()?
            } else {
                return Err(Box::new(Error::UnsufficientInput {}));
            };
            if let Some(start_str) = splitted_line.next() {
                let time = parse_time(start_str)?;
                state.clockedit.set_start_time(i - 1, time)?;
            }
            Ok(())
        }));
        terminal.register_command("end", Box::new(|state: &mut ClockEditCli, line: &str, _| {
            let mut splitted_line = line.split(" ");
            splitted_line.next();
            let i = if let Some(index) = splitted_line.next() {
                index.parse::<usize>()?
            } else {
                return Err(Box::new(Error::UnsufficientInput {}));
            };
            if let Some(end_str) = splitted_line.next() {
                let time = parse_time(end_str)?;
                state.clockedit.set_end_time(i - 1, time)?;
            }
            Ok(())
        }));
        terminal.register_command("enddate", Box::new(|state: &mut ClockEditCli, line: &str, _| {
            let mut splitted_line = line.split(" ");
            splitted_line.next();
            let i = if let Some(index) = splitted_line.next() {
                index.parse::<usize>()?
            } else {
                return Err(Box::new(Error::UnsufficientInput {}));
            };
            if let Some(end_str) = splitted_line.next() {
                let date = parse_date(end_str)?;
                state.clockedit.set_end_date(i - 1, date)?;
            }
            Ok(())
        }));
        terminal.register_command("apply", Box::new(|state: &mut ClockEditCli, _, callbacks| {
            state.apply_result = ExitAction::Apply;
            callbacks.exit();
            Ok(())
        }));
        terminal.register_command("ls", Box::new(|state: &mut ClockEditCli, _, callbacks| {
            for (clock, i) in state.clockedit.clocks.iter().zip(1..) {
                let start = &clock.start;
                let end = clock.end.map(|end| format!("{}", end)).unwrap_or("(none)".to_string());
                let comment = clock.comment.clone().map(|comment| comment).unwrap_or("(none)".to_string());
                let task_str = if let Some(task_id) = clock.task_id {
                    let path = state.doc.path(&task_id);
                    join_strings(path.iter()
                        .map(|task_id| state.doc.get(task_id))
                        .filter_map(|task| task.ok())
                        .map(|task| task.title.clone()), " -> ")
                } else {
                    "(none)".to_string()
                };
                callbacks.println(&format!("{}: {} - {}:\n Task: {}\n Comment: {}", i, start, end, task_str, comment));
            }
            Ok(())
        }));
    }
}
