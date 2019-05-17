use uuid::Uuid;
use serde::{Serialize, Deserialize};
use super::tasks::*;
use super::clock::*;
use super::error::*;
use std::io::Write;
use std::io::Read;
use std::fs::File;
use std::collections::HashMap;
use std::rc::Rc;
use std::path::Path;
use snafu::ResultExt;
use chrono::prelude::*;
use super::statics::*;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Doc {
    pub map: HashMap<Uuid, Rc<Task>>,

    #[serde(default)]
    pub clocks: HashMap<Uuid, Rc<Clock>>,
    pub current_clock: Option<Uuid>,
    pub root: Uuid
}

impl Doc {
    pub fn new() -> Doc {
        let mut map = HashMap::new();
        let root = Task::new();
        let root_id = root.id.clone();
        map.insert(root_id.clone(), Rc::new(root));
        Doc {
            map: map,
            clocks: HashMap::default(),
            current_clock: None,
            root: root_id
        }
    }

    pub fn save(&self, path: impl AsRef<Path>) -> Result<()> {
        Ok(serde_json::to_writer(
            File::create(path).context(IO)?, self)
            .context(SerdeSerializationError)?)
    }

    pub fn load(path: impl AsRef<Path>) -> Result<Doc> {
        Ok(
            serde_json::from_reader(
                File::open(path).context(IO)?
            ).context(SerdeSerializationError)?
        )
    }

    pub fn get(&self, id: &Uuid) -> Rc<Task> {
        self.map.get(id).unwrap().clone()
    }

    pub fn get_root(&self) -> Rc<Task> {
        self.get(&self.root)
    }

    pub fn upsert(&mut self, task: Rc<Task>) {
        self.map.insert(task.id.clone(), task);
    }

    pub fn modify_task<F>(&mut self, id: &Uuid, func: F)
            where F: Fn(Rc<Task>) -> Rc<Task> {
        let task = self.get(id);
        let task = func(task);
        self.upsert(task);
    }

    pub fn add_subtask(&mut self, task: Rc<Task>, parent_ref: &Uuid) {
        self.modify_task(parent_ref, |mut parent| parent.add_child(task.id.clone()).clone() );
        self.upsert(task);
    }

    pub fn find_parent(&self, task_ref: &Uuid) -> Option<Uuid> {
        self.map.values().find(|task| task.children.iter().any(|child_id| child_id == task_ref)).map(|task| task.id.clone())
    }

    pub fn to_html(&self, task_ref: &Uuid) -> String {
        let mut html = String::new();
        let task = self.get(task_ref);
        html.push_str("<!doctype html><html><head><link rel=\"stylesheet\" href=\"https://stackpath.bootstrapcdn.com/bootstrap/4.3.1/css/bootstrap.min.css\" integrity=\"sha384-ggOyR0iXCbMQv3Xipma34MD+dH/1fQ784/j6cY/iJTQUOhcWr7x9JvoRxT2MZw1T\" crossorigin=\"anonymous\"></head><body><div class=\"container\">");

        let mut breadcrumb_item_opn = Some(task_ref.clone());
        let mut breadcrumb_data = Vec::new();
        loop {
            if let Some(breadcrumb_item) = breadcrumb_item_opn {
                breadcrumb_data.push(breadcrumb_item.clone());
                breadcrumb_item_opn = self.find_parent(&breadcrumb_item);
            } else {
                break;
            }
        }
        breadcrumb_data.iter().rev().zip(1..).for_each(|(breadcrumb_ref, i)| {
            let task = self.get(breadcrumb_ref);
            if i > 1 {
                html.push_str(" -> ");
            }
            html.push_str(&format!("<a href=\"{}.html\">{}</a>", breadcrumb_ref, task.title));
        });

        let (done, all_subtasks) = self.progress_summary(task_ref);
        html.push_str(&format!("[{}/{}]", done, all_subtasks));

        html.push_str(&markdown::to_html(&task.body));
        html.push_str("<hr/>");
        html.push_str("<ul>");
        for child in task.children.iter() {
            let child_task = self.get(child);
            html.push_str("<li><a href=\"");
            html.push_str(&child.to_string());
            html.push_str(".html\">");
            html.push_str(&if let Some(ref progress) = child_task.progress { 
                progress.to_string()
            } else {
                String::new()
            });
            html.push_str(" ");
            html.push_str(&child_task.title);
            html.push_str("</a></li>");
        }
        html.push_str("</ul>");
        html.push_str("</div></body></html>");
        html
    }

    pub fn progress_summary(&self, task_ref: &Uuid) -> (i32, i32) {
        self.get(task_ref)
            .children.iter()
            .filter_map(|child| self.get(child).progress)
            .fold((0, 0), |(acc_done, acc_sum), progress| (
                acc_done + if progress.done() { 1 } else { 0 },
                acc_sum + 1
            ))
    }

    pub fn clock(&self, clock_ref: &Uuid) -> Result<Rc<Clock>> {
        self.clocks.get(clock_ref).map(|item| item.clone()).ok_or(Error::ClockNotFound {})
    }

    pub fn upsert_clock(&mut self, clock: Rc<Clock>) {
        self.clocks.insert(clock.id.clone(), clock);
    }

    pub fn clock_out(&mut self) -> Result<bool> {
        if let Some(ref clock_ref) = self.current_clock {
            let mut clock = self.clock(clock_ref)?;
            clock.set_end(Local::now());
            self.upsert_clock(clock);
            self.current_clock = None;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    pub fn clock_new(&mut self) -> Result<Rc<Clock>> {
        self.clock_out()?;
        let clock = Rc::new(Clock {
            id: Uuid::new_v4(),
            start: Local::now(),
            end: None,
            comment: None,
            task_id: None
        });
        self.upsert_clock(clock.clone());
        self.current_clock = Some(clock.id.clone());
        Ok(clock)
    }

    pub fn clock_assign(&mut self, task_ref: Uuid) -> Result<()> {
        if let Some(ref clock_ref) = self.current_clock {
            let mut clock = self.clock(clock_ref)?;
            clock.set_task_id(task_ref);
            self.upsert_clock(clock);
        }
        Ok(())
    }

    pub fn clock_comment(&mut self, comment: impl ToString) -> Result<()> {
        if let Some(ref clock_ref) = self.current_clock {
            let mut clock = self.clock(clock_ref)?;
            clock.set_comment(comment.to_string());
            self.upsert_clock(clock);
        }
        Ok(())
    }

    pub fn task_clock(&self, task_ref: &Uuid) -> Vec<Rc<Clock>> {
        self.clocks.values()
            .filter(|clock| clock.task_id == Some(*task_ref))
            .map(|clock| clock.clone()).collect()
    }
    
    pub fn day_clock(&self, date: Date<Local>) -> Vec<Rc<Clock>> {
        self.clocks.values()
            .filter(|clock| clock.start.date() == date)
            .map(|clock| clock.clone()).collect()
    }
}




pub fn rec_print(doc: &mut Doc, task_id: &Uuid, level: usize, max_depth: usize) {
    if level >= max_depth {
        return;
    }
    let task = doc.get(task_id);
    for _ in 0..level {
        print!(" ");
    }
    print!("* ");
    println!("{} {}", task.id, task.title);
    for child_id in task.children.iter() {
        rec_print(doc, child_id, level + 1, max_depth);
    }
}

pub fn dump_html_rec(doc: &Doc, dir: &Path, task_ref: &Uuid) -> Result<()> {
    let task = doc.get(task_ref);
    for child in task.children.iter() {
        dump_html_rec(doc, dir, child)?;
    }
    let task_html = doc.to_html(task_ref);
    let filename = dir.join(format!("{}.html", task_ref));
    println!("{}", filename.to_str().unwrap_or("N/A"));
    let mut html_file = File::create(filename).context(IO)?;
    html_file.write_all(task_html.as_bytes()).context(IO)?;
    Ok(())
}

pub fn dump_html(doc: &Doc, dir: &Path, task_ref: &Uuid) -> Result<()> {
    std::fs::create_dir_all(dir).context(IO)?;
    dump_html_rec(doc, dir, task_ref)?;
    let filename = dir.join(format!("index.html"));
    let mut index_file = File::create(filename).context(IO)?;
    index_file.write_all(b"<!doctype html><html><head></head><body><a href=\"").context(IO)?;
    index_file.write_all(task_ref.to_string().as_bytes()).context(IO)?;
    index_file.write_all(b".html\">Index</a></body></html>").context(IO)?;
    Ok(())
}

pub fn vim_edit_task(mut task: Rc<Task>) -> Rc<Task> {
    {   
        let mut out = File::create(&*TASK_FILE).expect("Could not create .task file");
        out.write_all(task.title.as_bytes()).expect("Couldn't write title to .task file");
        out.write_all("\n\n".as_bytes()).expect("Couldn't write newlines to .task file");
        out.write_all(task.body.as_bytes()).expect("Couldn't write body to .task file");
    }
    subprocess::Exec::cmd("vi").arg(&*TASK_FILE).join().unwrap();
    let mut content = String::new();
    {
        let mut input = File::open(&*TASK_FILE).expect("Could not open .task file");
        input.read_to_string(&mut content).expect("Couldn't read .task file");
    }
    let mut lines = content.lines();
    let title = lines.next().expect("Couldn't extract title");
    let body = lines.fold(String::new(), |mut acc: String, item| { acc.push_str(&item); acc.push('\n'); acc});
    task.set_title(title).set_body(body.trim());
    task
}

