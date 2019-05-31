use std::collections::HashMap;
use crate::error::*;


pub type Result<T, E=Box<dyn std::error::Error>> = std::result::Result<T, E>;

pub type Func<T, C: CliCallbacks<T>> = Box<Fn(&mut T, &str, &mut C) -> Result<()>>;

pub enum CliInputResult {
    Value(String),
    Termination,
}

pub trait CliStateCallback<T> {
    fn pre_exec(&mut self, state: &mut T, command: &str) {}
    fn post_exec(&mut self, state: &mut T, command: &str) {}
}

pub trait CliCallbacks<T> : CliStateCallback<T> {
    fn print(&mut self, text: &str);
    fn println(&mut self, text: &str) {
        self.print(&format!("{}\n", text));
    }

    fn read_line(&mut self, prompt: &str) -> CliInputResult;
    fn edit_string(&mut self, text: String) -> String;

    fn exit(&mut self);
    fn is_exit(&self) -> bool;
}

pub struct CliCallbackHolder<'a, T, T2, C2: CliStateCallback<T2>> {
    callbacks: &'a mut CliCallbacks<T>,
    state_callbacks: C2,
    exit: bool,
    t2: std::marker::PhantomData<T2>,
}
impl<'a, T, T2, C2: CliStateCallback<T2>> CliCallbackHolder<'a, T, T2, C2> {
    pub fn new(callbacks: &'a mut CliCallbacks<T>, state_callbacks: C2) -> Self {
        CliCallbackHolder {
            callbacks,
            state_callbacks,
            exit: false,
            t2: std::marker::PhantomData,
        }
    }

    
}

pub fn new_cli_with_callbacks<T: Sized, C: CliCallbacks<T>, T2: Sized, C2: CliStateCallback<T2>>(callbacks: &mut C, initial_state: T2, state_callbacks: C2) -> Cli<T2, CliCallbackHolder<T, T2, C2>> {
    Cli {
        state: initial_state,
        commands: HashMap::new(),
        callbacks: CliCallbackHolder::new(callbacks, state_callbacks),
    }
}

impl<'a, T, T2, C2: CliStateCallback<T2>> CliStateCallback<T2> for CliCallbackHolder<'a, T, T2, C2> {
    fn pre_exec(&mut self, state: &mut T2, command: &str) {
        self.state_callbacks.pre_exec(state, command)
    }
    fn post_exec(&mut self, state: &mut T2, command: &str) {
        self.state_callbacks.post_exec(state, command)
    }
}
impl<'a, T, T2, C2: CliStateCallback<T2>> CliCallbacks<T2> for CliCallbackHolder<'a, T, T2, C2> {
    fn print(&mut self, text: &str) {
        self.callbacks.print(text)
    }
    fn println(&mut self, text: &str) {
        self.callbacks.println(text)
    }

    fn read_line(&mut self, prompt: &str) -> CliInputResult {
        self.callbacks.read_line(prompt)
    }
    fn edit_string(&mut self, text: String) -> String {
        self.callbacks.edit_string(text)
    }

    fn exit(&mut self) {
        self.exit = true;
    }

    fn is_exit(&self) -> bool {
        self.exit
    }
}

pub struct Cli<T: Sized, C: CliCallbacks<T>> {
    pub state: T,
    pub commands: HashMap<String, Func<T, C>>,
    pub callbacks: C,
}

impl<T: Sized, C: CliCallbacks<T>> Cli<T, C> {
    pub fn new(initial_state: T, callbacks: C) -> Cli<T, C> {
        Cli {
            state: initial_state,
            commands: HashMap::new(),
            callbacks,
        }
    }

    pub fn new_with_callbacks<T2: Sized, C2: CliStateCallback<T2>>(&mut self, initial_state: T2, state_callbacks: C2) -> Cli<T2, CliCallbackHolder<T, T2, C2>> {
        Cli {
            state: initial_state,
            commands: HashMap::new(),
            callbacks: CliCallbackHolder::new(&mut self.callbacks, state_callbacks),
        }
    }

    pub fn run_command(&mut self, line: &str) -> Result<()> {
        if let Some(command) = line.trim().split(" ").next() {
            if let Some(func) = self.commands.get(command) {
                func(&mut self.state, line.trim(), &mut self.callbacks)
            } else {
                Err(Box::new(CliError::CommandNotFound { command: command.to_string() }))
            }
        } else {
            Err(Box::new(CliError::Empty))
        }
    }

    pub fn run_loop(&mut self, prompt: &str) {
        loop {
            match self.callbacks.read_line(prompt) {
                CliInputResult::Value(input) => {
                    self.callbacks.pre_exec(&mut self.state, &input);
                    match self.run_command(&input) {
                        Ok(()) => {},
                        Err(err) => self.callbacks.println(&format!("Error: {}", err))
                    }
                    self.callbacks.post_exec(&mut self.state, &input);
                    /*if Autosave::OnCommand == terminal.state.autosave {
                        if let Err(err) = terminal.state.doc.save(&main_file_path) {
                            self.callbacks.println(&format!("Couldn't save the file, sorry: {}", err));
                        }
                    }
                    rl.add_history_entry(input);*/
                    if self.callbacks.is_exit() {
                        break
                    }
                },
                CliInputResult::Termination => break,
            }
        }
    }

    pub fn register_command(&mut self, command: impl ToString, func: Func<T, C>) {
        self.commands.insert(command.to_string(), func);
    }

    pub fn remove_command(&mut self, command: &str) -> Option<Func<T, C>> {
        self.commands.remove(command)
    }
}