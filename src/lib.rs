#[macro_use]
extern crate lazy_static;
extern crate chrono;
extern crate flate2;
extern crate regex;
extern crate threadpool;

use chrono::{DateTime, FixedOffset};
use flate2::read::GzDecoder;
use regex::Regex;
use std::cmp::min;
use std::collections::HashMap;
use std::error::Error;
use std::fs;
use std::io::prelude::Read;
use std::sync::mpsc;

pub fn run(paths: Vec<String>, n: usize, config: Config) -> Result<(), Box<dyn Error>> {
    let path_log_map;
    if paths.len() > 1 {
        path_log_map = parse_files(paths, config);
    } else {
        let log_path = &paths[0];
        let metadata = fs::metadata(log_path).unwrap();
        if metadata.is_file() {
            path_log_map = parse_file(log_path, config);
        } else if metadata.is_dir() {
            path_log_map = parse_dir(log_path, config);
        } else {
            path_log_map = HashMap::new();
        }
    }

    let stats = compute_stats(&path_log_map);
    print_stats(stats, n);
    Ok(())
}

fn is_media_path(path: &String) -> bool {
    let re = Regex::new(r"^.*\.(txt|xml|css|js|jpg|png|gif|svg|ico|otf)$").unwrap();
    return re.is_match(&path.to_lowercase());
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

fn print_stats(counts: Vec<(usize, String)>, top_n: usize) {
    let n = min(top_n, counts.len());
    for (count, path) in &counts[..n] {
        println!("{}: {}", count, path);
    }
}

fn parse_dir(log_path: &String, config: Config) -> HashMap<String, Vec<ParsedLine>> {
    let paths = fs::read_dir(log_path).unwrap();
    let log_paths = paths
        .map(|entry| String::from(entry.unwrap().path().to_str().unwrap()).clone())
        .collect::<Vec<String>>();
    parse_files(log_paths, config)
}

fn parse_files(log_paths: Vec<String>, config: Config) -> HashMap<String, Vec<ParsedLine>> {
    let (tx, rx) = mpsc::channel();
    let file_count = log_paths.len();
    let num_pool_workers = 4;
    let pool = threadpool::ThreadPool::new(num_pool_workers);
    for entry in log_paths {
        let tx = mpsc::Sender::clone(&tx);
        let conf = config.clone();
        pool.execute(move || {
            let group_by_path = parse_file(&entry, conf);
            tx.send(group_by_path).unwrap();
        });
    }
    let mut path_log_map: HashMap<String, Vec<ParsedLine>> = HashMap::new();
    // FIXME: What if one of the thread crashes?
    for mut received in rx.iter().take(file_count) {
        for (path, logs) in &mut received {
            let all_path_logs = path_log_map.entry(path.to_string()).or_insert(Vec::new());
            all_path_logs.append(logs);
        }
    }
    path_log_map
}

fn read_file(path: &String) -> String {
    let contents: String;
    if path.ends_with(".gz") {
        let mut gz = GzDecoder::new(fs::File::open(path).unwrap());
        let mut s = String::new();
        gz.read_to_string(&mut s).unwrap();
        contents = s;
    } else {
        contents = fs::read_to_string(path).expect("Something went wrong reading the file");
    }

    contents
}

fn parse_file(log_path: &String, config: Config) -> HashMap<String, Vec<ParsedLine>> {
    println!("Parsing logs from {}", log_path);
    let contents = read_file(log_path);
    parse_string(contents, config)
}

fn parse_string(contents: String, config: Config) -> HashMap<String, Vec<ParsedLine>> {
    let mut group_by_path = HashMap::new();
    for line in contents.lines() {
        let mut parsed = parse_line(line);
        let path = parsed.path.to_string();
        if (config.include_errors || parsed.status == 200)
            && (config.include_media || !is_media_path(&path))
        {
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
    // FIXME: It might be better to do actual URL parsing at some point
    path: String,
    status: i32,
    referrer: String,
    user_agent: String,
}

#[derive(Debug, Clone)]
pub struct Config {
    pub include_errors: bool,
    pub include_media: bool,
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
        let config = Config {
            include_media: true,
            include_errors: false,
        };
        let parsed_content = parse_string(String::from(log_contents), config);
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

        let config = Config {
            include_media: true,
            include_errors: false,
        };
        let parsed_content = parse_string(String::from(log_contents), config);
        let stats = compute_stats(&parsed_content);
        assert_eq!(stats[0], (2, String::from("/index.xml")));
    }

}
