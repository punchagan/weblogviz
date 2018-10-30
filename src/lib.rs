#[macro_use]
extern crate lazy_static;
extern crate chrono;
extern crate regex;

use std::error::Error;
use std::fs;
use regex::Regex;
use chrono::{DateTime, FixedOffset};
use std::collections::HashMap;

pub fn run(log_path: &String) -> Result<(), Box<dyn Error>> {
    println!("Parsing logs from {}", log_path);

    let contents = fs::read_to_string(log_path).expect("Something went wrong reading the file");
    let group_by_path = parse_file(contents);
    let stats: Vec<(usize, String)> = compute_stats(group_by_path);
    print_stats(stats);

    Ok(())
}

fn compute_stats(path_map: HashMap<String, Vec<ParsedLine>>) -> Vec<(usize, String)> {
    let mut counts: Vec<(usize, String)> = Vec::new();
    for (key, value) in path_map {
        counts.push((value.len(), key));
    }
    // Reverse sort
    counts.sort_by(|a, b| b.cmp(a));
    counts
}

fn print_stats(counts: Vec<(usize, String)>) {
    for (count, path) in &counts[..10] {
        println!("{}: {}", count, path);
    }
}

fn parse_file(contents: String) -> HashMap<String, Vec<ParsedLine>> {
    let mut group_by_path = HashMap::new();
    for line in contents.lines() {
        let parsed = parse_line(line);
        if parsed.status == 200 {
            let path = parsed.path.to_string();
            let parsed_lines = group_by_path.entry(path).or_insert(vec![]);
            parsed_lines.push(parsed);
        }
    }
    group_by_path
}

#[derive(Debug)]
struct ParsedLine {
    ip: String,
    date: DateTime<FixedOffset>,
    path: String,
    status: i32,
    referrer: String,
    user_agent: String,
}

fn parse_line<'a>(line: &'a str) -> ParsedLine {
    lazy_static! {
        static ref RE: Regex = Regex::new(r#"^(\d{1,3}\.\d{1,3}\.\d{1,3}\.\d{1,3}) - - \[(.*?)\] "([A-Z]+) (.*?) HTTP/*.*" (\d{3}) (\d+) "(.*?)" "(.*?)"$"#).unwrap();
    }
    let captures = RE.captures(line).unwrap();

    ParsedLine {
        ip: String::from(captures.get(1).unwrap().as_str()),
        date: DateTime::parse_from_str(captures.get(2).unwrap().as_str(), "%d/%b/%Y:%H:%M:%S %z")
            .unwrap(),
        path: String::from(captures.get(4).unwrap().as_str()),
        status: String::from(captures.get(5).unwrap().as_str())
            .parse()
            .unwrap(),
        referrer: String::from(captures.get(7).unwrap().as_str()),
        user_agent: String::from(captures.get(8).unwrap().as_str()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_one_line() {
        let log_line = "49.206.4.211 - - [29/Oct/2018:07:35:39 -0700] \"GET / HTTP/1.1\" 200 14643 \"http://google.com\" \"Mozilla/5.0 (X11; Ubuntu; Linux x86_64; rv:64.0) Gecko/20100101 Firefox/64.0\"";

        assert_eq!("49.206.4.211", parse_line(log_line).ip);
        assert_eq!(
            DateTime::parse_from_rfc3339("2018-10-29T07:35:39-07:00").unwrap(),
            parse_line(log_line).date
        );
        assert_eq!("/", parse_line(log_line).path);
        assert_eq!(200, parse_line(log_line).status);
        assert_eq!("http://google.com", parse_line(log_line).referrer);
        assert_eq!(
            "Mozilla/5.0 (X11; Ubuntu; Linux x86_64; rv:64.0) Gecko/20100101 Firefox/64.0",
            parse_line(log_line).user_agent
        );
    }
}
