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
