#[macro_use]
extern crate lazy_static;

pub mod statics;
pub mod error;
pub mod tasks;
pub mod clock;
pub mod doc;
pub mod state;
pub mod cli;

pub use std::env::var;
pub use uuid::Uuid;
pub use std::io::Write;
pub use std::path::Path;
pub use chrono::Local;
pub use std::rc::Rc;

pub use error::*;
pub use tasks::*;
pub use doc::*;
pub use state::*;