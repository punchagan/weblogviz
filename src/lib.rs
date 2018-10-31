#[macro_use]
extern crate lazy_static;
extern crate chrono;
extern crate regex;
extern crate threadpool;

use std::error::Error;
use std::fs;
use regex::Regex;
use chrono::{DateTime, FixedOffset};
use std::collections::HashMap;
use std::sync::mpsc;

pub fn run(log_path: &String) -> Result<(), Box<dyn Error>> {
    let metadata = fs::metadata(log_path).unwrap();
    if metadata.is_file() {
        print_file_stats(log_path);
    } else if metadata.is_dir() {
        print_dir_stats(log_path);
    }
    Ok(())
}

fn print_dir_stats(log_path: &String) {
    println!("Parsing logs from {}", log_path);
    let (tx, rx) = mpsc::channel();
    let mut file_count = 0;
    let num_pool_workers = 4;
    let pool = threadpool::ThreadPool::new(num_pool_workers);
    for entry in fs::read_dir(log_path).unwrap() {
        file_count += 1;
        let tx = mpsc::Sender::clone(&tx);
        pool.execute(move || {
            let log_path = String::from(entry.unwrap().path().to_str().unwrap());
            let contents =
                fs::read_to_string(log_path).expect("Something went wrong reading the file");
            let group_by_path = parse_file(contents);
            tx.send(group_by_path).unwrap();
        });
    }
    let mut path_log_map: HashMap<String, Vec<ParsedLine>> = HashMap::new();
    for mut received in rx.iter().take(file_count) {
        for (path, logs) in &mut received {
            let all_path_logs = path_log_map.entry(path.to_string()).or_insert(Vec::new());
            all_path_logs.append(logs);
        }
    }
    print_stats(compute_stats(&path_log_map));
}

fn print_file_stats(log_path: &String) {
    println!("Parsing logs from {}", log_path);
    let contents = fs::read_to_string(log_path).expect("Something went wrong reading the file");
    let group_by_path = parse_file(contents);
    let stats: Vec<(usize, String)> = compute_stats(&group_by_path);
    print_stats(stats);
}

fn compute_stats(path_map: &HashMap<String, Vec<ParsedLine>>) -> Vec<(usize, String)> {
    let mut counts: Vec<(usize, String)> = Vec::new();
    for (key, value) in path_map {
        counts.push((value.len(), key.to_string()));
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
        let path = parsed.path.to_string();
        let parsed_lines = group_by_path.entry(path).or_insert(vec![]);
        parsed_lines.push(parsed);
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
        let parsed_line = parse_line(log_line);

        assert_eq!("49.206.4.211", parsed_line.ip);
        assert_eq!(
            DateTime::parse_from_rfc3339("2018-10-29T07:35:39-07:00").unwrap(),
            parsed_line.date
        );
        assert_eq!("/", parsed_line.path);
        assert_eq!(200, parsed_line.status);
        assert_eq!("http://google.com", parsed_line.referrer);
        assert_eq!(
            "Mozilla/5.0 (X11; Ubuntu; Linux x86_64; rv:64.0) Gecko/20100101 Firefox/64.0",
            parsed_line.user_agent
        );
    }

    #[test]
    fn parse_multiple_lines() {
        let log_contents = "49.206.4.211 - - [29/Oct/2018:07:35:39 -0700] \"GET / HTTP/1.1\" 200 14643 \"http://google.com\" \"Mozilla/5.0 (X11; Ubuntu; Linux x86_64; rv:64.0) Gecko/20100101 Firefox/64.0\"
54.166.138.147 - - [29/Oct/2018:07:39:20 -0700] \"GET /rss.xml HTTP/1.1\" 301 3977 \"-\" \"curl\"
54.166.138.147 - - [29/Oct/2018:07:39:20 -0700] \"GET /index.xml HTTP/1.1\" 200 42318 \"-\" \"curl\"
34.239.107.223 - - [29/Oct/2018:07:40:44 -0700] \"HEAD /rss.xml HTTP/1.1\" 301 3258 \"-\" \"Slackbot 1.0 (+https://api.slack.com/robots)\"
195.159.176.226 - - [28/Oct/2018:11:05:15 +0530] \"GET /index.xml HTTP/1.1\" 200 42318 \"-\" \"Gwene/1.0 (The gwene.org rss-to-news gateway)\"";

        let parsed_content = parse_file(String::from(log_contents));
        assert_eq!(parsed_content.keys().len(), 2);
        assert_eq!(parsed_content.contains_key("/rss.xml"), false);
        assert_eq!(parsed_content.get("/index.xml").unwrap().len(), 2);
        assert_eq!(parsed_content.get("/").unwrap().len(), 1);
    }

    #[test]
    fn count_parsed_lines() {
        let log_contents = "49.206.4.211 - - [29/Oct/2018:07:35:39 -0700] \"GET / HTTP/1.1\" 200 14643 \"http://google.com\" \"Mozilla/5.0 (X11; Ubuntu; Linux x86_64; rv:64.0) Gecko/20100101 Firefox/64.0\"
54.166.138.147 - - [29/Oct/2018:07:39:20 -0700] \"GET /rss.xml HTTP/1.1\" 301 3977 \"-\" \"curl\"
54.166.138.147 - - [29/Oct/2018:07:39:20 -0700] \"GET /index.xml HTTP/1.1\" 200 42318 \"-\" \"curl\"
34.239.107.223 - - [29/Oct/2018:07:40:44 -0700] \"HEAD /rss.xml HTTP/1.1\" 301 3258 \"-\" \"Slackbot 1.0 (+https://api.slack.com/robots)\"
195.159.176.226 - - [28/Oct/2018:11:05:15 +0530] \"GET /index.xml HTTP/1.1\" 200 42318 \"-\" \"Gwene/1.0 (The gwene.org rss-to-news gateway)\"";

        let parsed_content = parse_file(String::from(log_contents));
        let stats = compute_stats(&parsed_content);
        assert_eq!(stats[0], (2, String::from("/index.xml")));
    }

}
