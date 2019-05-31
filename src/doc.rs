//! Holding data which are serialized and stored to disk.

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
use crate::cli::CliCallbacks;

/// Holding data which are serialized and stored to disk.
/// 
/// # Example
/// 
/// ```
/// use todoapp3::doc::Doc;
/// use todoapp3::tasks::Task;
/// use todoapp3::TaskMod;
/// use todoapp3::tasks::Progress;
/// use std::rc::Rc;
/// 
/// // Initialize the doc.
/// let mut doc = Doc::new();
/// 
/// // The doc now contains one single root task.  Lets edit its title and
/// // body text.
/// doc.modify_task(&doc.root.clone(), |task| {
///     task
///         .set_title("Title of the root task")
///         .set_body("Some text");
/// });
/// 
/// // Now lets access the roots title.
/// assert_eq!(doc.get_root().title, "Title of the root task");
/// 
/// // Add lets generate a new task and set some title as well.
/// let mut child1 = Rc::new(Task::new());
/// child1.set_title("I'm the child");
/// 
/// // New lets add this text under the root.
/// let root_ref = doc.root.clone();
/// doc.add_subtask(child1, &root_ref);
/// 
/// // Now lets read the title of doc's first child.
/// {
///     // Get the new root
///     let root = doc.get_root();
///     // Get the child.  `children` is a Vec of IDs which are
///     // used to get the task.
///     let child = doc.get(&root.children[0]);
///     // Read the title
///     assert_eq!("I'm the child", child.title);
/// 
/// }
/// 
/// // Now lets add a body to the child
/// {
///     // Get the root
///     let root = doc.get_root();
///     // Get the child's id
///     let child_id = root.children[0];
///     // Modify the body
///     doc.modify_task(&child_id, |child| {
///         child.set_body("This is the child's body");
///     });
///     // Read the body
///     assert_eq!("This is the child's body", doc.get(&child_id).body);
/// }
/// 
/// 
/// // Now lets work on the child
/// {
///     // Get the root
///     let root = doc.get_root();
///     // Get the child's id
///     let child_id = root.children[0];
/// 
///     // Let's make it a task and assign TODO to it
///     doc.modify_task(&child_id, |child| {
///         child.set_progress(Progress::Todo);
///     });
/// 
///     // Start working and start tracking the time.
///     doc.clock_new().expect("Create a new clock");
///     
///     // Lets point the current clock to the child task.
///     doc.clock_assign(child_id).expect("Assign clock");
/// 
///     // Do some work.  And when done, mark it as done.
///     doc.modify_task(&child_id, |child| {
///         child.set_progress(Progress::Done);
///     });
/// 
///     // And finally clock out.
///     doc.clock_out().expect("Clocking out");
/// }
/// 
/// ```
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Doc {
    pub map: HashMap<Uuid, Rc<Task>>,

    #[serde(default)]
    pub clocks: HashMap<Uuid, Rc<Clock>>,
    pub current_clock: Option<Uuid>,
    pub root: Uuid
}

impl Doc {
    /// Create a new, empty document.
    /// 
    /// It is initialized with one empty root task which contains
    /// a random UUID.
    /// 
    /// # Example
    /// 
    /// ```
    /// use todoapp3::doc::Doc;
    /// let doc = Doc::new();
    /// ```
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

    /// Write the content to into the specified file.
    pub fn save(&self, path: impl AsRef<Path>) -> Result<()> {
        Ok(serde_json::to_writer(
            File::create(path).context(IO)?, self)
            .context(SerdeSerializationError)?)
    }

    /// Load the document of hte given path and return a new doc.
    /// 
    /// # Error
    /// Produces an error if there are IO issues or if the file format
    /// couldn't be parsed.
    pub fn load(path: impl AsRef<Path>) -> Result<Doc> {
        Ok(
            serde_json::from_reader(
                File::open(path).context(IO)?
            ).context(SerdeSerializationError)?
        )
    }

    /// Load task which contains the given id.
    /// 
    /// # Panic
    /// Panics if no task does exist.
    pub fn get(&self, id: &Uuid) -> Result<Rc<Task>> {
        self.map.get(id).map(|task| task.clone()).ok_or(Error::TaskUuidNotFound {})
    }

    /// Get the root task.
    pub fn get_root(&self) -> Result<Rc<Task>> {
        self.get(&self.root)
    }

    /// Adds or replaces the given task.
    /// 
    /// The task is identified by its id.
    pub fn upsert(&mut self, task: Rc<Task>) {
        self.map.insert(task.id.clone(), task);
    }

    /// Modify the task with a function or closure
    /// 
    /// # Panic
    /// Panics if no id for the task exists.
    pub fn modify_task<F>(&mut self, id: &Uuid, func: F) -> Result<()>
            where F: Fn(&mut Rc<Task>) -> Result<(), Box<std::error::Error>> {
        let mut task = self.get(id)?;
        Rc::make_mut(&mut task);
        func(&mut task).context(CustomError)?;
        self.upsert(task);
        Ok(())
    }

    /// Add a new task as child of the given parent id.
    /// 
    /// # Panic
    /// Panics if the id of the parent task doesn't exist.
    pub fn add_subtask(&mut self, task: Rc<Task>, parent_ref: &Uuid) -> Result<()> {
        self.modify_task(parent_ref, |parent| { parent.add_child(task.id.clone()); Ok(()) })?;
        self.upsert(task);
        Ok(())
    }

    /// Return the parent of the given task.
    /// 
    /// It will be None, if not found.
    pub fn find_parent(&self, task_ref: &Uuid) -> Option<Uuid> {
        self.map.values().find(|task| task.children.iter().any(|child_id| child_id == task_ref)).map(|task| task.id.clone())
    }

    /// Checks if the first given task is a child or the second task or if it's
    /// the task itself.
    pub fn is_in_hierarchy_of(&self, child_task: &Uuid, parent_task: &Uuid) -> bool {
        let mut tmp_task = child_task.clone();
        let mut counter = 0;
        loop {
            // In case of a loop (which hopefully doesn't happen), break after
            // 200 iterations.
            if counter == 200 {
                return false;
            }
            counter += 1;
            if tmp_task == *parent_task {
                return true;
            }
            if let Some(new_parent) = self.find_parent(&tmp_task) {
                tmp_task = new_parent.clone();
            } else {
                return false;
            }
        }
    }

    /// Get the i_th child of the given task
    /// 
    /// Returns None if the i is out of range.
    pub fn task_child(&self, task_id: &Uuid, i: usize) -> Option<Uuid> {
        let task = self.get(task_id).ok()?;
        if i < task.children.len() {
            Some(task.children[i].clone())
        } else {
            None
        }
    }

    /// Get the first child of the given task which has the prefix in the title.
    /// 
    /// Returns None if prefix matches no children.
    pub fn task_child_prefix(&self, task_id: &Uuid, prefix: &str) -> Option<Uuid> {
        let task = self.get(task_id).ok()?;
        let prefix = prefix.to_lowercase().replace(" ", "_");
        for child in task.children.iter() {
            let child_task = self.get(child).ok()?;
            let title = child_task.title.to_lowercase().replace(" ", "_");
            if title.starts_with(&prefix) {
                return Some(child.clone());
            }
        }
        None
    }

    /// Get all tasks, from the given one to the root.
    pub fn path(&self, task_ref: &Uuid) -> Vec<Uuid> {
        let mut res = Vec::new();
        let mut task_ref_opt = Some(task_ref.clone());
        loop {
            if let Some(task_ref) = task_ref_opt {
                res.push(task_ref.clone());
                task_ref_opt = self.find_parent(&task_ref);
            } else {
                break;
            }
        }
        res
    }

    /// Return a String which contains a html code which represents the givent task.
    /// 
    /// # Panic
    /// Panics if the task id is not found.
    pub fn to_html(&self, task_ref: &Uuid) -> Result<String> {
        let mut html = String::new();
        let task = self.get(task_ref)?;
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
            if let Ok(task) = self.get(breadcrumb_ref) {
                if i > 1 {
                    html.push_str(" -> ");
                }
                html.push_str(&format!("<a href=\"{}.html\">{}</a>", breadcrumb_ref, task.title));
            }
        });

        let (done, all_subtasks) = self.progress_summary(task_ref)?;
        html.push_str(&format!("[{}/{}]", done, all_subtasks));

        html.push_str(&markdown::to_html(&task.body));
        html.push_str("<hr/>");
        html.push_str("<ul>");
        for child in task.children.iter() {
            let child_task = self.get(child)?;
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
        Ok(html)
    }

    /// Summary how many children are done vs how many have any progress state.
    /// 
    /// It counts the children which have a progress assigned which indicates that
    /// the task is not done in the first tuple entry and the count of children
    /// which contain any progress field.  Actually, this is the current progress
    /// state of the task: todo/all.
    pub fn progress_summary(&self, task_ref: &Uuid) -> Result<(i32, i32)> {
        Ok(self.get(task_ref)?
            .children.iter()
            .filter_map(|child_ref| self.get(child_ref).ok())
            .filter_map(|child| child.progress)
            .fold((0, 0), |(acc_done, acc_sum), progress| (
                acc_done + if progress.done() { 1 } else { 0 },
                acc_sum + 1
            )))
    }

    /// Get the clock which is under the name.
    /// 
    /// # Error
    /// Returns an error if a clock wasn't found under the name.
    pub fn clock(&self, clock_ref: &Uuid) -> Result<Rc<Clock>> {
        self.clocks.get(clock_ref).map(|item| item.clone()).ok_or(Error::ClockNotFound {})
    }

    /// Insert or replace the clock.
    pub fn upsert_clock(&mut self, clock: Rc<Clock>) {
        self.clocks.insert(clock.id.clone(), clock);
    }

    /// Stops clocking time.
    /// 
    /// # Error
    /// If the internal state is incorrect and the current_clock
    /// references to a clock which doesn't exist, it will return
    /// an error.
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

    /// Generate a new clock which starts at the time it was called.
    /// 
    /// # Error
    /// Return an error on an internal error if the clock out doesn't
    /// work.
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

    /// Assign the given task to the active clock.
    /// 
    /// # Error
    /// It will return an error if the internal state is wrong and the current
    /// clock id cannot be found.
    pub fn clock_assign(&mut self, task_ref: Uuid) -> Result<()> {
        if let Some(ref clock_ref) = self.current_clock {
            let mut clock = self.clock(clock_ref)?;
            clock.set_task_id(task_ref);
            self.upsert_clock(clock);
        }
        Ok(())
    }

    /// Set the comment of the active clock.
    /// 
    /// # Error
    /// It will return an error if the internal state is wrong and the current
    /// clock id cannot be found.
    pub fn clock_comment(&mut self, comment: impl ToString) -> Result<()> {
        if let Some(ref clock_ref) = self.current_clock {
            let mut clock = self.clock(clock_ref)?;
            clock.set_comment(comment.to_string());
            self.upsert_clock(clock);
        }
        Ok(())
    }

    /// Get the clocks assigned to the given task.
    pub fn task_clock(&self, task_ref: &Uuid) -> Vec<Rc<Clock>> {
        self.clocks.values()
            .filter(|clock| clock.task_id == Some(*task_ref))
            .map(|clock| clock.clone()).collect()
    }
    
    /// Get the clocks for the given date.
    pub fn day_clock(&self, date: Date<Local>, main_task: impl Into<Option<Uuid>>) -> Vec<Rc<Clock>> {
        let main_task = main_task.into();
        self.clocks.values()
            .filter(|clock| clock.start.date() == date)
            .filter(|clock|
                if let Some(clock_task) = clock.task_id {
                    if let Some(main_task) = main_task {
                        self.is_in_hierarchy_of(&clock_task, &main_task)
                    } else { true }
                } else { true })
            .map(|clock| clock.clone()).collect()
    }

    /// Get the clocks of the given date.
    pub fn range_clock(&self, start: Date<Local>, end: Date<Local>, main_task: impl Into<Option<Uuid>>) -> Vec<Rc<Clock>> {
        let main_task = main_task.into();
        self.clocks.values()
            .filter(|clock| clock.start.date() >= start && clock.start.date() <= end)
            .filter(|clock|
                if let Some(clock_task) = clock.task_id {
                    if let Some(main_task) = main_task {
                        self.is_in_hierarchy_of(&clock_task, &main_task)
                    } else { true }
                } else { true })
            .cloned().collect()
    }
}




pub fn rec_print<T>(doc: &mut Doc, task_id: &Uuid, level: usize, max_depth: usize, callbacks: &mut CliCallbacks<T>) -> Result<()> {
    if level >= max_depth {
        return Ok(());
    }
    let task = doc.get(task_id)?;
    for _ in 0..level {
        callbacks.print(" ");
    }
    callbacks.print("* ");
    callbacks.println(&format!("{} {}", task.id, task.title));
    for child_id in task.children.iter() {
        rec_print(doc, child_id, level + 1, max_depth, callbacks)?;
    }
    Ok(())
}

pub fn dump_html_rec<T>(doc: &Doc, dir: &Path, task_ref: &Uuid, callbacks: &mut CliCallbacks<T>) -> Result<()> {
    let task = doc.get(task_ref)?;
    for child in task.children.iter() {
        dump_html_rec(doc, dir, child, callbacks)?;
    }
    let task_html = doc.to_html(task_ref)?;
    let filename = dir.join(format!("{}.html", task_ref));
    callbacks.println(&format!("{}", filename.to_str().unwrap_or("N/A")));
    let mut html_file = File::create(filename).context(IO)?;
    html_file.write_all(task_html.as_bytes()).context(IO)?;
    Ok(())
}

pub fn dump_html<T>(doc: &Doc, dir: &Path, task_ref: &Uuid, callbacks: &mut CliCallbacks<T>) -> Result<()> {
    std::fs::create_dir_all(dir).context(IO)?;
    dump_html_rec(doc, dir, task_ref, callbacks)?;
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

