use std::collections::HashMap;
use uuid::Uuid;
use serde::{Serialize, Deserialize};
use std::io::Write;
use std::io::Read;
use std::fs::File;
use std::path::Path;
use snafu::{Snafu, ResultExt, Backtrace, ErrorCompat, ensure};
use std::rc::Rc;
use std::process::{Command, Stdio};

const TASK_FILE: &str = ".task.md";


mod terminal;

#[derive(Debug, Snafu)]
enum Error {
    #[snafu(display("IO Error: {}", source))]
    IO { source: std::io::Error },

    #[snafu(display("Serde Serialize Error: {}", source))]
    SerdeSerializationError { source: serde_json::error::Error }
}

type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
enum Progress {
    Todo, Work, Done
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
struct Task {
    id: Uuid,
    title: String,
    body: String,
    children: Vec<Uuid>,
    progress: Option<Progress>
}

impl Task {
    pub fn new() -> Task {
        let root_id = Uuid::new_v4();
        Task {
            id: root_id.clone(),
            title: String::new(),
            body: String::new(),
            children: Vec::new(),
            progress: None
        }
    }
}

trait TaskMod {
    fn set_title(&mut self, title: impl ToString) -> &mut Self;
    fn set_body(&mut self, body: impl ToString) -> &mut Self;
    fn set_children(&mut self, children: Vec<Uuid>) -> &mut Self;
    fn add_child(&mut self, child: Uuid) -> &mut Self;
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
    fn set_progress(&mut self, progress: Progress) -> &mut Self {
        Rc::make_mut(self).progress = Some(progress);
        self
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct Doc {
    map: HashMap<Uuid, Rc<Task>>,
    root: Uuid
}

impl Doc {
    fn new() -> Doc {
        let mut map = HashMap::new();
        let root = Task::new();
        let root_id = root.id.clone();
        map.insert(root_id.clone(), Rc::new(root));
        Doc {
            map: map,
            root: root_id
        }
    }

    fn save(&self, path: impl AsRef<Path>) -> Result<()> {
        Ok(serde_json::to_writer(
            File::create(path).context(IO)?, self)
            .context(SerdeSerializationError)?)
    }

    fn load(path: impl AsRef<Path>) -> Result<Doc> {
        Ok(
            serde_json::from_reader(
                File::open(path).context(IO)?
            ).context(SerdeSerializationError)?
        )
    }

    fn get(&self, id: &Uuid) -> Rc<Task> {
        self.map.get(id).unwrap().clone()
    }

    fn get_root(&self) -> Rc<Task> {
        self.get(&self.root)
    }

    fn upsert(&mut self, task: Rc<Task>) {
        self.map.insert(task.id.clone(), task);
    }

    fn modify_task<F>(&mut self, id: &Uuid, func: F)
            where F: Fn(Rc<Task>) -> Rc<Task> {
        let mut task = self.get(id);
        let task = func(task);
        self.upsert(task);
    }

    fn add_subtask(&mut self, task: Rc<Task>, parent_ref: &Uuid) {
        self.modify_task(parent_ref, |mut parent| parent.add_child(task.id.clone()).clone() );
        self.upsert(task);
    }
}

fn vim_edit_task(mut task: Rc<Task>) -> Rc<Task> {
    {   
        let mut out = File::create(TASK_FILE).expect("Could not create .task file");
        out.write_all(task.title.as_bytes()).expect("Couldn't write title to .task file");
        out.write_all("\n\n".as_bytes()).expect("Couldn't write newlines to .task file");
        out.write_all(task.body.as_bytes()).expect("Couldn't write body to .task file");
    }
    subprocess::Exec::cmd("vi").arg(TASK_FILE).join().unwrap();
    let mut content = String::new();
    {
        let mut input = File::open(TASK_FILE).expect("Could not open .task file");
        input.read_to_string(&mut content).expect("Couldn't read .task file");
    }
    let mut lines = content.lines();
    let title = lines.next().expect("Couldn't extract title");
    let body = lines.fold(String::new(), |mut acc: String, item| { acc.push_str(&item); acc.push('\n'); acc});
    task.set_title(title).set_body(body.trim());
    task
}

#[derive(Debug)]
struct State {
    doc: Doc,
    wt: Uuid,
    parents: Vec<Uuid>,
    path: String
}

fn main() {
    let doc = Doc::new();
    let state = State {
        wt: doc.root.clone(),
        doc: doc,
        parents: Vec::new(),
        path: "tasks.json".to_string()
    };
    let mut terminal = terminal::Terminal::new(state);
    terminal.register_command("exit", Box::new(|_, _| true));
    terminal.register_command("debug", Box::new(|state, _| { println!("{:?}", state); false }));
    terminal.register_command("ls", Box::new(|state: &mut State, _| {
        let task = state.doc.get(&state.wt);
        println!("{}", task.title);
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
        false
    }));
    terminal.register_command("ed", Box::new(|state: &mut State, _| {
        let task = vim_edit_task(state.doc.get(&state.wt));
        state.doc.upsert(task);
        false
    }));
    terminal.register_command("add", Box::new(|state: &mut State, _| {
        let task = vim_edit_task(Rc::new(Task::new()));
        state.doc.add_subtask(task, &state.wt);
        false
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
        false
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
        false
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
            } 
        } else {
            state.wt = state.doc.root.clone();
            state.parents = Vec::new();
        }
        false
    }));
    terminal.register_command("todo", Box::new(|state: &mut State, _| {
        let mut task = state.doc.get(&state.wt);
        task.set_progress(Progress::Todo);
        state.doc.upsert(task);
        false
    }));
    terminal.register_command("work", Box::new(|state: &mut State, _| {
        let mut task = state.doc.get(&state.wt);
        task.set_progress(Progress::Work);
        state.doc.upsert(task);
        false
    }));
    terminal.register_command("done", Box::new(|state: &mut State, _| {
        let mut task = state.doc.get(&state.wt);
        task.set_progress(Progress::Work);
        state.doc.upsert(task);
        false
    }));

    let mut input = String::new();
    loop {
        std::io::stdin().read_line(&mut input).expect("Error while reading user input");
        let exit = terminal.run_command(&input);
        if exit {
            break;
        }
        input.clear();
    }
}
