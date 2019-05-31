use crate::doc::*;
use crate::clock::*;
use crate::error::*;
use crate::DurationPrint;
use crate::cli::CliCallbacks;
use std::rc::Rc;
use chrono::Local;
use chrono::TimeZone;
use chrono::Date;

pub fn fold_strings<'a>(sep: &'a str) -> impl FnMut(String, (String, usize)) -> String + 'a {
    move | mut acc, (item, i) | {
        if i > 1 {
            acc.push_str(sep);
        }
        acc.push_str(&item);
        acc
    }
}

pub fn join_strings(strings: impl Iterator<Item=String>, sep: &str) -> String {
    strings
        .zip(1..)
        .fold(String::new(), fold_strings(sep))
}

pub fn parse_time(string: &str) -> chrono::ParseResult<chrono::NaiveTime> {
    let time = if let Ok(time) = chrono::NaiveTime::parse_from_str(string, "%H:%M:%S") {
        time
    } else {
        chrono::NaiveTime::parse_from_str(string, "%H:%M")?
    };
    Ok(time)
}

pub fn display_clocks<T>(clocks: &[Rc<Clock>], doc: &Doc, callbacks: &mut CliCallbacks<T>) {
    let overall_duration = clocks.iter()
        .map(|clock| clock.duration())
        .fold(chrono::Duration::zero(), |acc, new| acc + new);
    let mut clocks = clocks.to_vec();
    clocks.sort();
    let mut current_day = None;
    let mut day_duration = chrono::Duration::zero();
    for clock in clocks.iter() {
        let start = &clock.start;
        let end = clock.end.map(|end| format!("{}", end)).unwrap_or_else(|| "(none)".to_string());
        let comment = clock.comment.clone().map(|comment| comment).unwrap_or_else(|| "(none)".to_string());
        let task_str = if let Some(task_id) = clock.task_id {
            let path = doc.path(&task_id);
            join_strings(path.iter().rev()
                .map(|task_id| doc.get(task_id))
                .filter_map(|task| task.ok())
                .map(|task| task.title.clone()), " -> ")
        } else {
            "(none)".to_string()
        };
        let day = start.date();
        if Some(day) != current_day {
            callbacks.println(&format!("--- {} ---", day));
        }
        callbacks.println(&format!("{} - {}:\n Task: {}\n Comment: {}", start, end, task_str, comment));
        if Some(day) != current_day {
            if current_day.is_some() {
                callbacks.println(&format!("Day duration: {}", day_duration.print()));
                callbacks.println("");
            }
            day_duration = chrono::Duration::zero();
            current_day = Some(day);
        }
        day_duration = day_duration + clock.duration();
    }
    callbacks.println(&format!("Day duration: {}", day_duration.print()));
    callbacks.println("");
    callbacks.println(&format!("Overall duration in time range: {}", overall_duration.print()));
}

pub fn parse_date(date_str: &str) -> CliResult<Date<Local>> {
    Ok(if date_str.starts_with('-') {
        match (&date_str[1..]).parse::<i64>() {
            Ok(offset) => Local::today() - chrono::Duration::days(offset),
            Err(err) => return Err(CliError::ParseError { msg: format!("{}", err) }),
        }
    } else if let Ok(naive_date) = chrono::NaiveDate::parse_from_str(date_str, "%Y-%m-%d") {
        if let Some(date) = Local.from_local_date(&naive_date).earliest() {
            date
        } else {
            return Err(CliError::ParseError { msg: "Couldn't apply timezone".to_string() })
        }
    } else {
        return Err(CliError::ParseError { msg: "Couldn't parse argument format".to_string() })
    })
}
