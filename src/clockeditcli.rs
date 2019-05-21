use crate::terminal::*;
use crate::clockedit::*;
use crate::error::*;
use crate::doc::*;
use crate::helper::*;
use std::io::Write;

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

impl<'a> ClockEditCli<'a> {
    pub fn run(self) -> Self {
        let mut terminal = Terminal::new(self);
        terminal.register_command("cancel", Box::new(|_, _| {
            Ok(true)
        }));
        terminal.register_command("start", Box::new(|state: &mut ClockEditCli, line: &str| {
            let mut splitted_line = line.split(" ");
            splitted_line.next();
            let i = if let Some(index) = splitted_line.next() {
                index.parse::<usize>()?
            } else {
                return Err(Box::new(Error::UnsufficientInput {}));
            };
            if let Some(start_str) = splitted_line.next() {
                let time = if let Ok(time) = chrono::NaiveTime::parse_from_str(start_str, "%H:%M:%S") {
                    time
                } else {
                    chrono::NaiveTime::parse_from_str(start_str, "%H:%M")?
                };
                state.clockedit.set_start_time(i - 1, time)?;
            }
            Ok(false)
        }));
        terminal.register_command("end", Box::new(|state: &mut ClockEditCli, line: &str| {
            let mut splitted_line = line.split(" ");
            splitted_line.next();
            let i = if let Some(index) = splitted_line.next() {
                index.parse::<usize>()?
            } else {
                return Err(Box::new(Error::UnsufficientInput {}));
            };
            if let Some(end_str) = splitted_line.next() {
                let time = if let Ok(time) = chrono::NaiveTime::parse_from_str(end_str, "%H:%M:%S") {
                    time
                } else {
                    chrono::NaiveTime::parse_from_str(end_str, "%H:%M")?
                };
                state.clockedit.set_end_time(i - 1, time)?;
            }
            Ok(false)
        }));
        terminal.register_command("apply", Box::new(|state: &mut ClockEditCli, _| {
            state.apply_result = ExitAction::Apply;
            Ok(true)
        }));
        terminal.register_command("ls", Box::new(|state: &mut ClockEditCli, _| {
            for (clock, i) in state.clockedit.clocks.iter().zip(1..) {
                let start = &clock.start;
                let end = clock.end.map(|end| format!("{}", end)).unwrap_or("(none)".to_string());
                let comment = clock.comment.clone().map(|comment| comment).unwrap_or("(none)".to_string());
                let task_str = if let Some(task_id) = clock.task_id {
                    let path = state.doc.path(&task_id);
                    join_strings(path.iter()
                        .map(|task_id| state.doc.get(task_id))
                        .map(|task| task.title.clone()), " -> ")
                } else {
                    "(none)".to_string()
                };
                println!("{}: {} - {}:\n Task: {}\n Comment: {}", i, start, end, task_str, comment);
            }
            Ok(false)
        }));

        let mut input = String::new();
        loop {
            print!("clock edit > ");
            std::io::stdout().flush().expect("Couldn't flush stdout");
            std::io::stdin().read_line(&mut input).expect("Error while reading user input");
            let exit = terminal.run_command(&input);
            if exit {
                break;
            }
            input.clear();
        }
        terminal.state
    }
}