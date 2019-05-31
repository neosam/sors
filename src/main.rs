#[macro_use]
extern crate lazy_static;

pub mod statics;
pub mod error;
pub mod tasks;
pub mod clock;
pub mod doc;
pub mod state;
pub mod cli;
pub mod clockedit;
pub mod clockeditcli;
pub mod helper;

use rustyline::error::ReadlineError;
use rustyline::Editor;

use std::env::var;
use std::io::Write;
use std::path::Path;
use chrono::Local;
use std::rc::Rc;

use error::*;
use tasks::*;
use doc::*;
use state::*;
use clockeditcli::*;
use helper::*;
use cli::*;
use std::fs::File;
use std::io::Read;
use crate::statics::*;



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

struct TerminalCallback {
    rl: Editor<()>,
    exit: bool,
    main_save_path: String,
}
impl TerminalCallback {
    pub fn new(main_save_path: String) -> Self {
        let mut rl = Editor::<()>::new();
        if rl.load_history(&*statics::HISTORY_FILE).is_err() {
            println!("No previous history.");
        }
        TerminalCallback {
            rl,
            main_save_path,
            exit: false,
        }
    }
}

impl CliStateCallback<State> for TerminalCallback {
    //fn pre_exec(&mut self, state: &mut T) {}
    fn post_exec(&mut self, state: &mut State, command: &str) {
        if Autosave::OnCommand == state.autosave {
            if let Err(err) = state.doc.save(&self.main_save_path) {
                self.println(&format!("Couldn't save the file, sorry: {}", err));
            }
        }
        self.rl.add_history_entry(command);
    }
}

impl CliCallbacks<State> for TerminalCallback {
    fn print(&mut self, text: &str) {
        print!("{}", text);
    }
    fn println(&mut self, text: &str) {
        println!("{}", text);
    }

    fn read_line(&mut self, prompt: &str) -> CliInputResult {
        match self.rl.readline(prompt) {
            Ok(input) => CliInputResult::Value(input),
            Err(ReadlineError::Eof) => CliInputResult::Termination,
            Err(ReadlineError::Interrupted) => CliInputResult::Termination,
            Err(err) => {
                println!("Error: {}", err);
                CliInputResult::Termination
            }
        }
    }
    fn edit_string(&mut self, text: String) -> String {
        {   
            let mut out = File::create(&*TASK_FILE).expect("Could not create .task file");
            out.write_all(text.as_bytes()).expect("Couldn't write title to .task file");
        }
        subprocess::Exec::cmd("vi").arg(&*TASK_FILE).join().unwrap();
        let mut content = String::new();
        {
            let mut input = File::open(&*TASK_FILE).expect("Could not open .task file");
            input.read_to_string(&mut content).expect("Couldn't read .task file");
        }
        content
    }

    fn exit(&mut self) {
        self.exit = true;
        if let Err(err) = self.rl.save_history(&*statics::HISTORY_FILE) {
            println!("Failed to save history: {}", err);
        }
    }

    fn is_exit(&self) -> bool {
        self.exit
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
    let mut terminal = cli::Cli::new(state, TerminalCallback::new(main_file_path));
    terminal.register_command("exit", Box::new(|_, _, response| {
        response.exit();
        Ok(())
    }));
    terminal.register_command("debug", Box::new(|state, _, response| { 
        response.println(&format!("{:?}", state));
        Ok(())
    }));
    terminal.register_command("ls", Box::new(|state: &mut State, _, response| {
        let task = state.doc.get(&state.wt)?;
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
            if let Ok(task) = state.doc.get(breadcrumb_ref) {
                if i > 1 {
                    response.print(" -> ");
                }
                response.print(&task.title);
            }
        });
        let (done, all_subtasks) = state.doc.progress_summary(&task.id)?;
        response.println(&format!("  [{}/{}]", done, all_subtasks));
        response.println("");
        response.println(&format!("{}", task.body));
        response.println(&format!("--- Children: "));
        for (child_id, i) in task.children.iter().zip(1..) {
            let child = state.doc.get(child_id)?;
            let progress_str = if let Some(progress) = &child.progress {
                progress.to_string()
            } else {
                String::new()
            };
            response.println(&format!("{}: {} {}", i, progress_str, child.title));
        }
        Ok(())
    }));
    terminal.register_command("ed", Box::new(|state: &mut State, _, callbacks| {
        let task = vim_edit_task(state.doc.get(&state.wt)?, callbacks)?;
        state.doc.upsert(task);
        Ok(())
    }));
    terminal.register_command("add", Box::new(|state: &mut State, _, callbacks| {
        let task = vim_edit_task(Rc::new(Task::new()), callbacks)?;
        state.doc.add_subtask(task, &state.wt)?;
        Ok(())
    }));
    terminal.register_command("save", Box::new(|state: &mut State, cmd: &str, _| {
        let mut split = cmd.split(" ");
        split.next();
        let filename = if let Some(filename) = split.next() {
            filename
        } else {
            &state.path
        };
        state.doc.save(filename).expect("Couldn't save the file");
        Ok(())
    }));
    terminal.register_command("load", Box::new(|state: &mut State, cmd: &str, _| {
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
        Ok(())
    }));
    terminal.register_command("cd", Box::new(|state: &mut State, cmd: &str, _| {
        let mut split = cmd.split(" ");
        split.next();
        if let Some(path) = split.next() {
            state.wt = state.uuid_for_path(path)
                .ok_or(CliError::ParseError { msg: "Couldn't resolve path".to_string() })?
        } else {
            state.wt = state.doc.root.clone();
            state.parents = Vec::new();
        }
        Ok(())
    }));
    terminal.register_command("todo", Box::new(|state: &mut State, _, _| {
        let mut task = state.doc.get(&state.wt)?;
        task.set_progress(Progress::Todo);
        state.doc.upsert(task);
        Ok(())
    }));
    terminal.register_command("work", Box::new(|state: &mut State, _, _| {
        let mut task = state.doc.get(&state.wt)?;
        task.set_progress(Progress::Work);
        state.doc.upsert(task);
        Ok(())
    }));
    terminal.register_command("done", Box::new(|state: &mut State, _, _| {
        let mut task = state.doc.get(&state.wt)?;
        task.set_progress(Progress::Done);
        state.doc.upsert(task);
        Ok(())
    }));
    terminal.register_command("id", Box::new(|state: &mut State, _, response| {
        let task = state.doc.get(&state.wt)?;
        response.println(&format!("Task ID: {}", task.id));
        Ok(())
    }));
    terminal.register_command("parent", Box::new(|state: &mut State, _, response| {
        let task = state.doc.get(&state.wt)?;
        if let Some(parent)  = state.doc.find_parent(&task.id) {
            response.println(&format!("Parent Task ID: {}", parent));
        }
        Ok(())
    }));
    terminal.register_command("rm", Box::new(|state: &mut State, cmd: &str, _| {
        let mut split = cmd.split(" ");
        split.next();
        if let Some(path) = split.next() {
            if let Some(child_id) = state.uuid_for_path(path) {
                if let Some(parent) = state.doc.find_parent(&child_id) {
                    let mut task = state.doc.get(&parent)?;
                    task.remove_child(&child_id);
                    state.doc.upsert(task);
                }
            }
        }
        Ok(())
    }));
    terminal.register_command("mv", Box::new(|state: &mut State, cmd: &str, _response| {
        let mut split = cmd.split(" ");
        split.next();
        let dest_id = {
            let path = split.next().ok_or(CliError::ParseError{ msg: "First path contains errors".to_string() })?;
            state.uuid_for_path(path).ok_or(Box::new(CliError::ParseError{ msg: "First path contains errors".to_string() }))?
        };
        let to_id = {
            let path = split.next().ok_or(CliError::ParseError{ msg: "First path contains errors".to_string() })?;
            state.uuid_for_path(path).ok_or(Box::new(CliError::ParseError{ msg: "First path contains errors".to_string() }))?
        };
        let parent_id = state.doc.find_parent(&dest_id)
            .ok_or(CliError::OtherError { msg: "Couldn't find parent".to_string()} )?;

        let mut parent = state.doc.get(&parent_id)?;
        parent.remove_child(&dest_id);
        state.doc.upsert(parent);
        let mut task = state.doc.get(&to_id)?;
        task.add_child(dest_id);
        state.doc.upsert(task);
        Ok(())
    }));
    terminal.register_command("outline", Box::new(|state: &mut State, cmd: &str, response| {
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
        rec_print(&mut state.doc, &state.wt, 0, max_depth, response)?;
        Ok(())
    }));
    terminal.register_command("html", Box::new(|state: &mut State, _, response| {
        dump_html(&state.doc, Path::new("html"), &state.wt, response)?;
        Ok(())
    }));
    terminal.register_command("reorder", Box::new(|state: &mut State, cmd: &str, _| {
        let mut split = cmd.split(" ");
        split.next();
        let idx_string: &str = split.next().ok_or(Error::UnsufficientInput {})?;
        let idx_from: usize = idx_string.parse()?;
        let idx_string: &str = split.next().ok_or(Error::UnsufficientInput {})?;
        let idx_to: usize = idx_string.parse()?;
        let mut task = state.doc.get(&state.wt)?;
        if idx_from > task.children.len() {
            return Err(Box::new(Error::ChildOutOfIndex {}));
        }
        if idx_to > task.children.len() {
            return Err(Box::new(Error::ChildOutOfIndex {}));
        }
        let from_id = task.children[idx_from - 1];
        task.remove_child(&from_id);
        task.insert_child(from_id, idx_to - 1);
        state.doc.upsert(task);
        Ok(())
    }));
    terminal.register_command("cli", Box::new(|state: &mut State, _, _| {
        state.doc.clock_new()?;
        state.doc.clock_assign(state.wt.clone())?;
        Ok(())
    }));
    terminal.register_command("cln", Box::new(|state: &mut State, _, _| {
        state.doc.clock_new()?;
        Ok(())
    }));
    terminal.register_command("cla", Box::new(|state: &mut State, _, _| {
        state.doc.clock_assign(state.wt.clone())?;
        Ok(())
    }));
    terminal.register_command("clo", Box::new(|state: &mut State, _, _| {
        state.doc.clock_out()?;
        Ok(())
    }));
    terminal.register_command("clc", Box::new(|state: &mut State, _, _response| {
        let mut comment = String::new();
        print!("Clock comment> ");
        std::io::stdout().flush()?;
        std::io::stdin().read_line(&mut comment)?;
        state.doc.clock_comment(comment.trim())?;
        Ok(())
    }));

    terminal.register_command("taskclock", Box::new(|state: &mut State, _, response| {
        let mut clocks = state.doc.task_clock(&state.wt);
        clocks.sort();
        display_clocks(&clocks, &state.doc, response);
        Ok(())
    }));
    terminal.register_command("dayclock", Box::new(|state: &mut State, cmd: &str, response| {
        let mut cmd_split = cmd.split(" ");
        cmd_split.next();
        let date = if let Some(param) = cmd_split.next() {
            parse_date(param)?
        } else {
            Local::today()
        };
        let mut clocks = state.doc.day_clock(date, state.wt);
        clocks.sort();
        display_clocks(&clocks, &state.doc, response);
        Ok(())
    }));
    terminal.register_command("autosave", Box::new(|state: &mut State, _, _| {
        state.autosave = Autosave::OnCommand;
        Ok(())
    }));
    terminal.register_command("noautosave", Box::new(|state: &mut State, _, _| {
        state.autosave = Autosave::ManualOnly;
        Ok(())
    }));
    terminal.register_command("cle", Box::new(|state: &mut State, cmd: &str, callbacks| {
        let mut cmd_split = cmd.split(" ");
        cmd_split.next();
        let date = if let Some(param) = cmd_split.next() {
            parse_date(param)?
        } else {
            Local::today()
        };
        let clockedit_state = {
            let clockedit_state = ClockEditCli {
                clockedit: state.doc.create_clock_edit(date),
                apply_result: ExitAction::Cancel,
                doc: &state.doc,
            };
            let mut clockedit_cli = new_cli_with_callbacks(callbacks, clockedit_state, ClockCallbacks);
            ClockEditCli::apply_commands(&mut clockedit_cli);
            clockedit_cli.run_loop("clockedit> ");
            clockedit_cli.state
        };
        if clockedit_state.apply_result == ExitAction::Apply {
            for clock in clockedit_state.clockedit.clocks.iter().cloned() {
                state.doc.upsert_clock(clock);
            }
        }
        Ok(())
    }));
    terminal.register_command("rangeclock", Box::new(|state: &mut State, cmd: &str, response| {
        let mut split_cmd = cmd.split(" ");
        split_cmd.next();
        if let Some(index_str) = split_cmd.next() {
            if let Ok(i) = index_str.parse() {
                let end = Local::today();
                let duration = chrono::Duration::days(i);
                let start = end - duration;
                let clocks = state.doc.range_clock(start, end, state.wt);
                display_clocks(&clocks, &state.doc, response);
            }
        }
        Ok(())
    }));
    terminal.run_loop("> ");
}
