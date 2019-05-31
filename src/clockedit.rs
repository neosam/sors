use crate::clock::Clock;
use crate::clock::ClockMod;
use crate::doc::Doc;
use crate::error::*;
use chrono::prelude::*;
use std::rc::Rc;

#[derive(Debug, Clone)]
pub struct ClockEdit {
    pub clocks: Vec<Rc<Clock>>
}

impl ClockEdit {
    pub fn get_clock(&self, i: usize) -> Result<Rc<Clock>> {
        self.clocks.get(i).cloned().ok_or(Error::ClockOutOfIndex {})
    }

    pub fn update_clock(&mut self, i: usize, clock: Rc<Clock>) -> Result<()> {
        if i < self.clocks.len() {
            self.clocks[i] = clock;
            Ok(())  
        } else {
            Err(Error::ClockOutOfIndex {})
        }
    }

    pub fn modify_clock(&mut self, i: usize, func: impl Fn(&mut Rc<Clock>)) -> Result<()> {
        let mut clock = self.get_clock(i)?;
        func(&mut clock);
        self.update_clock(i, clock)
    }

    pub fn set_duration(&mut self, i: usize, duration: chrono::Duration) -> Result<()> {
        self.modify_clock(i, move |clock: &mut Rc<Clock>| {
            let end = clock.start + duration;
            clock.set_end(end);
        })
    }

    pub fn set_start(&mut self, i: usize, start: DateTime<Local>) -> Result<()> {
        self.modify_clock(i, move |clock: &mut Rc<Clock>| {
            clock.set_start(start);
        })
    }
    pub fn set_start_time(&mut self, i: usize, start: NaiveTime) -> Result<()> {
        self.modify_clock(i, move |clock: &mut Rc<Clock>| {
            if let Some(new_start) = clock.start.date().and_time(start) {
                clock.set_start(new_start);
            }
        })
    }

    pub fn set_end(&mut self, i: usize, end: DateTime<Local>) -> Result<()> {
        self.modify_clock(i, move | clock: &mut Rc<Clock>| {
            clock.set_end(end);
        })
    }
    pub fn set_end_time(&mut self, i: usize, start: NaiveTime) -> Result<()> {
        self.modify_clock(i, move |clock: &mut Rc<Clock>| {
            if let Some(end) = clock.end {
                if let Some(new_start) = end.date().and_time(start) {
                    clock.set_end(new_start);
                }
            }
        })
    }
    pub fn set_end_date(&mut self, i: usize, new_end: Date<Local>) -> Result<()> {
        self.modify_clock(i, move |clock: &mut Rc<Clock>| {
            if let Some(end) = clock.end {
                if let Some(new_end) = new_end.and_time(end.time()) {
                    clock.set_end(new_end);
                }
            }
        })
    }
}

impl Doc {
    pub fn create_clock_edit(&self, date: Date<Local>) -> ClockEdit {
        let mut clocks: Vec<Rc<Clock>> = self.clocks.values()
            .filter(|clock| clock.start.date() == date)
            .cloned()
            .collect();
        clocks.sort();
        ClockEdit {
            clocks
        }
    }

    pub fn apply_clock_edit(&mut self, clock_edit: ClockEdit) {
        for clock in clock_edit.clocks.iter() {
            self.upsert_clock(clock.clone());
        }
    }
}
