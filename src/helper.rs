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