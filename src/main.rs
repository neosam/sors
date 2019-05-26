#[macro_use]
extern crate lazy_static;

pub mod statics;
pub mod error;
pub mod tasks;
pub mod clock;
pub mod doc;
pub mod state;
pub mod terminal;
pub mod clockedit;
pub mod clockeditcli;
pub mod helper;

use rustyline::error::ReadlineError;
use rustyline::Editor;

use std::env::var;
use uuid::Uuid;
use std::io::Write;
use std::path::Path;
use chrono::Local;
use chrono::TimeZone;
use std::rc::Rc;

use error::*;
use tasks::*;
use doc::*;
use state::*;
use clockeditcli::*;
use helper::*;

use snafu::{Snafu};





trait DurationPrint {
    fn print(&self) -> String;
}

impl DurationPrint for chrono::Duration {
    fn print(&self) -> String {
        format!("{}d {}h {}m {}s",
            self.num_days(),
            self.num_hours() % 24,
            self.num_minutes() % 60,
            self.num_seconds() % 60
        )
    }
}


fn main() {
    let main_file_path = format!("{}/.tasks.json", var("HOME").unwrap());
    let doc = Doc::load(&main_file_path).unwrap_or(Doc::new());
    let state = State {
        wt: doc.root.clone(),
        doc: doc,
        parents: Vec::new(),
        path: main_file_path.clone(),
        autosave: Autosave::ManualOnly,
    };
    let mut terminal = terminal::Terminal::new(state);
    terminal.register_command("exit", Box::new(|_, _| Ok(true)));
    terminal.register_command("debug", Box::new(|state, _| { println!("{:?}", state); Ok(false) }));
    terminal.register_command("ls", Box::new(|state: &mut State, _| {
        let task = state.doc.get(&state.wt);
        let mut breadcrumb_item_opn = Some(state.wt.clone());
        let mut breadcrumb_data = Vec::new();
        loop {
            if let Some(breadcrumb_item) = breadcrumb_item_opn {
                breadcrumb_data.push(breadcrumb_item.clone());
                breadcrumb_item_opn = state.doc.find_parent(&breadcrumb_item);
            } else {
                break;
            }
        }
        breadcrumb_data.iter().rev().zip(1..).for_each(|(breadcrumb_ref, i)| {
            let task = state.doc.get(breadcrumb_ref);
            if i > 1 {
                print!(" -> ");
            }
            print!("{}", task.title);
        });
        let (done, all_subtasks) = state.doc.progress_summary(&task.id);
        println!("  [{}/{}]", done, all_subtasks);
        println!();
        println!("{}", task.body);
        println!("--- Children: ");
        for (child_id, i) in task.children.iter().zip(1..) {
            let child = state.doc.get(child_id);
            let progress_str = if let Some(progress) = &child.progress {
                progress.to_string()
            } else {
                String::new()
            };
            println!("{}: {} {}", i, progress_str, child.title);
        }
        Ok(false)
    }));
    terminal.register_command("ed", Box::new(|state: &mut State, _| {
        let task = vim_edit_task(state.doc.get(&state.wt));
        state.doc.upsert(task);
        Ok(false)
    }));
    terminal.register_command("add", Box::new(|state: &mut State, _| {
        let task = vim_edit_task(Rc::new(Task::new()));
        state.doc.add_subtask(task, &state.wt);
        Ok(false)
    }));
    terminal.register_command("save", Box::new(|state: &mut State, cmd: &str| {
        let mut split = cmd.split(" ");
        split.next();
        let filename = if let Some(filename) = split.next() {
            filename
        } else {
            &state.path
        };
        state.doc.save(filename).expect("Couldn't save the file");
        Ok(false)
    }));
    terminal.register_command("load", Box::new(|state: &mut State, cmd: &str| {
        let mut split = cmd.split(" ");
        split.next();
        let filename = if let Some(filename) = split.next() {
            filename
        } else {
            &state.path
        };
        let doc = Doc::load(filename).expect("Couldn't save the file");
        let new_root = doc.root.clone();
        state.doc = doc;
        state.wt = new_root;
        Ok(false)
    }));
    terminal.register_command("cd", Box::new(|state: &mut State, cmd: &str| {
        let mut split = cmd.split(" ");
        split.next();
        if let Some(path) = split.next() {
            if let Some(child) = state.uuid_for_path(path) {
                state.wt = child.clone();
            } else {
                println!("Couldn't resolve path");
            }
        } else {
            state.wt = state.doc.root.clone();
            state.parents = Vec::new();
        }
        Ok(false)
    }));
    terminal.register_command("todo", Box::new(|state: &mut State, _| {
        let mut task = state.doc.get(&state.wt);
        task.set_progress(Progress::Todo);
        state.doc.upsert(task);
        Ok(false)
    }));
    terminal.register_command("work", Box::new(|state: &mut State, _| {
        let mut task = state.doc.get(&state.wt);
        task.set_progress(Progress::Work);
        state.doc.upsert(task);
        Ok(false)
    }));
    terminal.register_command("done", Box::new(|state: &mut State, _| {
        let mut task = state.doc.get(&state.wt);
        task.set_progress(Progress::Done);
        state.doc.upsert(task);
        Ok(false)
    }));
    terminal.register_command("id", Box::new(|state: &mut State, _| {
        let task = state.doc.get(&state.wt);
        println!("Task ID: {}", task.id);
        Ok(false)
    }));
    terminal.register_command("parent", Box::new(|state: &mut State, _| {
        let task = state.doc.get(&state.wt);
        println!("Parent Task ID: {}", state.doc.find_parent(&task.id).expect("Parent not found"));
        Ok(false)
    }));
    terminal.register_command("rm", Box::new(|state: &mut State, cmd: &str| {
        let mut split = cmd.split(" ");
        split.next();
        if let Some(path) = split.next() {
            if let Some(child_id) = state.uuid_for_path(path) {
                if let Some(parent) = state.doc.find_parent(&child_id) {
                    let mut task = state.doc.get(&parent);
                    //let child_id = state.doc.get(&state.wt).children[i - 1];
                    task.remove_child(&child_id);
                    state.doc.upsert(task);
                }
            }
        }
        Ok(false)
    }));
    terminal.register_command("mv", Box::new(|state: &mut State, cmd: &str| {
        let mut split = cmd.split(" ");
        split.next();
        let dest_id = {
            if let Some(path) = split.next() {
                state.uuid_for_path(path).ok_or(Box::new(CliError::ParseError{ msg: "First path contains errors".to_string() }))?
            } else {
                println!("No first path specified");
                return Ok(false)
            }
        };
        let to_id = {
            if let Some(path) = split.next() {
                state.uuid_for_path(path).ok_or(Box::new(CliError::ParseError{ msg: "First path contains errors".to_string() }))?
            } else {
                println!("No second UUID specified");
                return Ok(false)
            }
        };
        let parent_id = if let Some(parent_id) = state.doc.find_parent(&dest_id) {
            parent_id.clone()
        } else {
            println!("Couldn't find parents");
            return Ok(false);
        };
        let mut parent = state.doc.get(&parent_id);
        parent.remove_child(&dest_id);
        state.doc.upsert(parent);
        let mut task = state.doc.get(&to_id);
        task.add_child(dest_id);
        state.doc.upsert(task);
        Ok(false)
    }));
    terminal.register_command("outline", Box::new(|state: &mut State, cmd: &str| {
        let mut split = cmd.split(" ");
        split.next();
        let max_depth = if let Some(depth_str) = split.next() {
            if let Ok(max_depth) = depth_str.parse() {
                max_depth
            } else {
                1000
            }
        } else {
            1000
        };
        rec_print(&mut state.doc, &state.wt, 0, max_depth);
        Ok(false)
    }));
    terminal.register_command("html", Box::new(|state: &mut State, _| {
        if let Err(err) = dump_html(&state.doc, Path::new("html"), &state.wt) {
            println!("Couldn't dump html files: {}", err);
        }
        Ok(false)
    }));
    terminal.register_command("reorder", Box::new(|state: &mut State, cmd: &str| {
        let mut split = cmd.split(" ");
        split.next();
        let idx_string: &str = split.next().ok_or(Error::UnsufficientInput {})?;
        let idx_from: usize = idx_string.parse()?;
        let idx_string: &str = split.next().ok_or(Error::UnsufficientInput {})?;
        let idx_to: usize = idx_string.parse()?;
        let mut task = state.doc.get(&state.wt);
        let from_id = task.children[idx_from - 1];
        task.remove_child(&from_id);
        task.insert_child(from_id, idx_to - 1);
        state.doc.upsert(task);
        Ok(false)
    }));
    terminal.register_command("cli", Box::new(|state: &mut State, _| {
        state.doc.clock_new()?;
        state.doc.clock_assign(state.wt.clone())?;
        Ok(false)
    }));
    terminal.register_command("cln", Box::new(|state: &mut State, _| {
        state.doc.clock_new()?;
        Ok(false)
    }));
    terminal.register_command("cla", Box::new(|state: &mut State, _| {
        state.doc.clock_assign(state.wt.clone())?;
        Ok(false)
    }));
    terminal.register_command("clo", Box::new(|state: &mut State, _| {
        state.doc.clock_out()?;
        Ok(false)
    }));
    terminal.register_command("clc", Box::new(|state: &mut State, _| {
        let mut comment = String::new();
        print!("Clock comment> ");
        std::io::stdout().flush()?;
        std::io::stdin().read_line(&mut comment)?;
        state.doc.clock_comment(comment.trim())?;
        Ok(false)
    }));

    terminal.register_command("taskclock", Box::new(|state: &mut State, _| {
        let mut clocks = state.doc.task_clock(&state.wt);
        clocks.sort();
        display_clocks(&clocks, &state.doc);
        Ok(false)
    }));
    terminal.register_command("dayclock", Box::new(|state: &mut State, cmd: &str| {
        let mut cmd_split = cmd.split(" ");
        cmd_split.next();
        let date = if let Some(param) = cmd_split.next() {
            parse_date(param)?
        } else {
            Local::today()
        };
        let mut clocks = state.doc.day_clock(date, state.wt);
        clocks.sort();
        display_clocks(&clocks, &state.doc);
        Ok(false)
    }));
    terminal.register_command("autosave", Box::new(|state: &mut State, _| {
        state.autosave = Autosave::OnCommand;
        Ok(false)
    }));
    terminal.register_command("noautosave", Box::new(|state: &mut State, _| {
        state.autosave = Autosave::ManualOnly;
        Ok(false)
    }));
    terminal.register_command("cle", Box::new(|state: &mut State, cmd: &str| {
        let mut cmd_split = cmd.split(" ");
        cmd_split.next();
        let date = if let Some(param) = cmd_split.next() {
            parse_date(param)?
        } else {
            Local::today()
        };
        let clockeditcli = ClockEditCli {
            clockedit: state.doc.create_clock_edit(date),
            apply_result: ExitAction::Cancel,
            doc: &state.doc,
        };
        let final_value = clockeditcli.run();
        if final_value.apply_result == ExitAction::Apply {
            for clock in final_value.clockedit.clocks.iter().cloned() {
                state.doc.upsert_clock(clock);
            }
        }
        Ok(false)
    }));
    terminal.register_command("rangeclock", Box::new(|state: &mut State, cmd: &str| {
        let mut split_cmd = cmd.split(" ");
        split_cmd.next();
        if let Some(index_str) = split_cmd.next() {
            if let Ok(i) = index_str.parse() {
                let end = Local::today();
                let duration = chrono::Duration::days(i);
                let start = end - duration;
                let clocks = state.doc.range_clock(start, end, state.wt);
                display_clocks(&clocks, &state.doc);
            }
        }
        Ok(false)
    }));
    let mut rl = Editor::<()>::new();
    if rl.load_history(&*statics::HISTORY_FILE).is_err() {
        println!("No previous history.");
    }

    loop {
        match rl.readline("> ") {
            Ok(input) => {
                let exit = terminal.run_command(&input);
                if Autosave::OnCommand == terminal.state.autosave {
                    if let Err(err) = terminal.state.doc.save(&main_file_path) {
                        println!("Couldn't save the file, sorry: {}", err);
                    }
                }
                rl.add_history_entry(input);
                if exit {
                    break;
                }
            },
            Err(ReadlineError::Eof) => break,
            Err(ReadlineError::Interrupted) => break,
            Err(err) => println!("Error: {}", err),
        }
    }
    if let Err(err) = rl.save_history(&*statics::HISTORY_FILE) {
        println!("Failed to save history: {}", err);
    }
}
