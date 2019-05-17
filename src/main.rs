#[macro_use]
extern crate lazy_static;

mod statics;
mod error;
mod tasks;
mod clock;
mod doc;
mod state;
mod terminal;

use std::env::var;
use uuid::Uuid;
use std::io::Write;
use std::path::Path;
use chrono::Local;
use std::rc::Rc;

use error::*;
use tasks::*;
use doc::*;
use state::*;


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
    let doc = Doc::new();
    let state = State {
        wt: doc.root.clone(),
        doc: doc,
        parents: Vec::new(),
        path: format!("{}/.tasks.json", var("HOME").unwrap())
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
        if let Some(child) = split.next() {
            if child == ".." {
                if let Some(parent) = state.parents.pop() {
                    state.wt = parent;
                }
            } else if let Ok(i) = child.parse::<usize>() {
                let child_id = state.doc.get(&state.wt).children[i - 1];
                state.parents.push(state.wt.clone());
                state.wt = child_id;
            } else if let Ok(id) = Uuid::parse_str(child) {
                state.wt = id.clone();
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
        if let Some(child) = split.next() {
            if let Ok(i) = child.parse::<usize>() {
                let mut task = state.doc.get(&state.wt);
                let child_id = state.doc.get(&state.wt).children[i - 1];
                task.remove_child(&child_id);
                state.doc.upsert(task);
            }
        }
        Ok(false)
    }));
    terminal.register_command("mv", Box::new(|state: &mut State, cmd: &str| {
        let mut split = cmd.split(" ");
        split.next();
        let dest_id = {
            if let Some(dest_string) = split.next() {
                if let Ok(dest_id) = Uuid::parse_str(dest_string) {
                    dest_id.clone()
                } else {
                    println!("Error while parsing first uuid");
                    return Ok(false)
                }
            } else {
                println!("No first UUID specified");
                return Ok(false)
            }
        };
        let to_id = {
            if let Some(to_string) = split.next() {
                if let Ok(to_id) = Uuid::parse_str(to_string) {
                    to_id.clone()
                } else {
                    println!("Error while parsing second uuid");
                    return Ok(false)
                }
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
        let overall_duration = clocks.iter()
            .map(|clock| clock.duration())
            .fold(chrono::Duration::zero(), |acc, new| acc + new);
        for clock in clocks.iter() {
            let start = &clock.start;
            let end = clock.end.map(|end| format!("{}", end)).unwrap_or("(none)".to_string());
            let comment = clock.comment.clone().map(|comment| comment).unwrap_or("(none)".to_string());
            println!("{} - {}: {}", start, end, comment);
        }
        println!("{}", overall_duration.print());
        Ok(false)
    }));
    terminal.register_command("dayclock", Box::new(|state: &mut State, _| {
        let mut clocks = state.doc.day_clock(Local::today());
        clocks.sort();
        let overall_duration = clocks.iter()
            .map(|clock| clock.duration())
            .fold(chrono::Duration::zero(), |acc, new| acc + new);

        for clock in clocks.iter() {
            let start = &clock.start;
            let end = clock.end.map(|end| format!("{}", end)).unwrap_or("(none)".to_string());
            let comment = clock.comment.clone().map(|comment| comment).unwrap_or("(none)".to_string());
            println!("{} - {}: {}", start, end, comment);
        }
        println!("{}", overall_duration.print());
        Ok(false)
    }));    

    let mut input = String::new();
    loop {
        print!("> ");
        std::io::stdout().flush().expect("Couldn't flush stdout");
        std::io::stdin().read_line(&mut input).expect("Error while reading user input");
        let exit = terminal.run_command(&input);
        if exit {
            break;
        }
        input.clear();
    }
}
