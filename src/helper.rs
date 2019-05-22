use crate::doc::*;
use crate::clock::*;
use crate::DurationPrint;
use std::rc::Rc;

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

pub fn display_clocks(clocks: &Vec<Rc<Clock>>, doc: &Doc) {
    let overall_duration = clocks.iter()
        .map(|clock| clock.duration())
        .fold(chrono::Duration::zero(), |acc, new| acc + new);
    let mut clocks = clocks.clone();
    clocks.sort();
    let mut current_day = None;
    let mut day_duration = chrono::Duration::zero();
    for clock in clocks.iter() {
        let start = &clock.start;
        let end = clock.end.map(|end| format!("{}", end)).unwrap_or("(none)".to_string());
        let comment = clock.comment.clone().map(|comment| comment).unwrap_or("(none)".to_string());
        let task_str = if let Some(task_id) = clock.task_id {
            let path = doc.path(&task_id);
            join_strings(path.iter().rev()
                .map(|task_id| doc.get(task_id))
                .map(|task| task.title.clone()), " -> ")
        } else {
            "(none)".to_string()
        };
        let day = start.date();
        if Some(day) != current_day {
            println!("--- {} ---", day);
        }
        println!("{} - {}:\n Task: {}\n Comment: {}", start, end, task_str, comment);
        if Some(day) != current_day {
            if current_day.is_some() {
                println!("Day duration: {}", day_duration.print());
                println!();
            }
            day_duration = chrono::Duration::zero();
            current_day = Some(day);
        }
        day_duration = day_duration + clock.duration();
    }
    println!("Day duration: {}", day_duration.print());
    println!();
    println!("Overall duration in time range: {}", overall_duration.print());
}
