use std::collections::HashMap;


pub type Func<T> = Box<Fn(&mut T, &str) -> bool>;

pub struct Terminal<T: Sized> {
    state: T,
    commands: HashMap<String, Func<T>>
}

impl<T: Sized> Terminal<T> {
    pub fn new(initial_state: T) -> Terminal<T> {
        Terminal {
            state: initial_state,
            commands: HashMap::new()
        }
    }

    pub fn run_command(&mut self, line: &str) -> bool {
        if let Some(command) = line.trim().split(" ").next() {
            println!("Command: '{}'", command);
            if let Some(func) = self.commands.get(command) {
                func(&mut self.state, line.trim())
            } else {
                false
            }
        } else {
            false
        }
    }

    pub fn register_command(&mut self, command: impl ToString, func: Func<T>) {
        self.commands.insert(command.to_string(), func);
    }

    pub fn remove_command(&mut self, command: &str) -> Option<Func<T>> {
        self.commands.remove(command)
    }
}