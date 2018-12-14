#[macro_use]
extern crate lazy_static;
extern crate chrono;
extern crate flate2;
extern crate regex;
extern crate threadpool;

use chrono::{DateTime, FixedOffset, NaiveDate};
use flate2::read::GzDecoder;
use regex::Regex;
use std::cmp::min;
use std::collections::{BTreeMap, HashMap};
use std::error::Error;
use std::fs;
use std::io::prelude::Read;
use std::sync::mpsc;

pub fn run(paths: Vec<String>, n: usize, d: usize, config: Config) -> Result<(), Box<dyn Error>> {
    let log_db;
    if paths.len() > 1 {
        log_db = parse_files(paths, config);
    } else {
        let log_path = &paths[0];
        let metadata = fs::metadata(log_path).unwrap();
        if metadata.is_file() {
            log_db = parse_file(log_path, config);
        } else if metadata.is_dir() {
            log_db = parse_dir(log_path, config);
        } else {
            log_db = LogDB::new();
        }
    }

    let stats = compute_stats(&log_db);
    print_stats(stats, n);
    print_daily_hits(&log_db, d, n);
    Ok(())
}

fn is_media_path(path: &String) -> bool {
    lazy_static! {
        static ref media_re: Regex =
            Regex::new(r"^.*\.(txt|xml|css|js|jpg|png|gif|svg|ico|otf)$").unwrap();
    }
    return media_re.is_match(&path.to_lowercase());
}

fn is_crawler(user_agent: &String) -> bool {
    lazy_static! {
        static ref re: Regex = Regex::new(
            r"(https:|http:|Bot|bot|crawler|spider|compatible;|subscriber|Gwene|Zapier|Automattic|WhatsApp|curl|scraper|Wget|Python|Ruby|Go|Rome|Jersey|Emacs|\+collection@|Slack|Reeder|Twitter|requests|Apache-|perl|uatools)"
        ).unwrap();

    }
    return re.is_match(&user_agent);
}

fn compute_stats(log_db: &LogDB) -> Vec<(usize, String)> {
    let mut counts: Vec<(usize, String)> = Vec::new();
    for (key, value) in &log_db.by_path {
        counts.push((value.len(), key.to_string()));
    }
    // Reverse sort
    counts.sort_by(|a, b| b.cmp(a));
    counts
}

fn print_stats(counts: Vec<(usize, String)>, top_n: usize) {
    let n = min(top_n, counts.len());
    println!("URL paths with the most hits (overall) - Top {}", n);
    println!("# of hits:\tpath");
    for (count, path) in &counts[..n] {
        println!("{}:\t\t{}", count, path);
    }
    println!("##############################################");
}

fn print_daily_hits(log_db: &LogDB, days: usize, last_n: usize) {
    println!("Date:\t\t# of hits");
    let mut sorted_dates: Vec<NaiveDate> = log_db.by_date.keys().cloned().collect();
    sorted_dates.sort_by(|a, b| b.cmp(a));
    let n = min(last_n, sorted_dates.len());
    let d = min(days, sorted_dates.len());
    for date in &sorted_dates[..d] {
        println!("{}:\t{}", date, log_db.by_date.get(date).unwrap().len());
        let mut filtered_log_db = LogDB::new();
        for index in log_db.by_date.get(date).unwrap() {
            let parsed_line = log_db.logs[*index as usize].clone();
            filtered_log_db.insert_parsed_line(parsed_line);
        }
        print_stats(compute_stats(&filtered_log_db), n);
    }
}

fn parse_dir(log_path: &String, config: Config) -> LogDB {
    let paths = fs::read_dir(log_path).unwrap();
    let log_paths = paths
        .map(|entry| String::from(entry.unwrap().path().to_str().unwrap()).clone())
        .collect::<Vec<String>>();
    parse_files(log_paths, config)
}

fn parse_files(log_paths: Vec<String>, config: Config) -> LogDB {
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
    let mut log_db = LogDB::new();
    // FIXME: What if one of the thread crashes?
    for received in rx.iter().take(file_count) {
        log_db.merge(received);
    }
    log_db
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

fn parse_file(log_path: &String, config: Config) -> LogDB {
    println!("Parsing logs from {}", log_path);
    let contents = read_file(log_path);
    parse_string(contents, config)
}

fn parse_string(contents: String, config: Config) -> LogDB {
    let mut log_db = LogDB::new();
    for line in contents.lines() {
        let parsed = parse_line(line);
        if parsed.is_none() {
            println!("Skipping line: {}", line);
            continue;
        }
        let mut parsed = parsed.unwrap();
        if config.ignore_query_params {
            let path_fragments: Vec<&str> = parsed.path.split("?").collect();
            parsed.path = String::from(path_fragments[0]);
        }
        let path = parsed.path.to_string();
        if (config.include_errors || parsed.status == 200)
            && (config.include_media || !is_media_path(&path))
            && (config.include_crawlers || !is_crawler(&parsed.user_agent))
        {
            log_db.insert_parsed_line(parsed);
        }
    }
    log_db
}

#[derive(Debug, Clone)]
struct ParsedLine {
    ip: String,
    date: DateTime<FixedOffset>,
    // FIXME: It might be better to do actual URL parsing at some point
    path: String,
    status: i32,
    referrer: String,
    user_agent: String,
}

#[derive(Debug)]
struct LogDB {
    logs: Vec<ParsedLine>,
    by_path: HashMap<String, Vec<usize>>,
    by_date: BTreeMap<NaiveDate, Vec<usize>>,
}

impl LogDB {
    fn new() -> LogDB {
        LogDB {
            logs: Vec::new(),
            by_path: HashMap::new(),
            by_date: BTreeMap::new(),
        }
    }
    fn insert_parsed_line(&mut self, parsed_line: ParsedLine) {
        let index = self.logs.len();
        // Update by_path
        let path_map = self
            .by_path
            .entry(parsed_line.path.clone())
            .or_insert(vec![]);
        path_map.push(index);
        // Update by_date
        let date_map = self
            .by_date
            .entry(parsed_line.date.naive_utc().date())
            .or_insert(vec![]);
        date_map.push(index);
        // Update .logs
        &self.logs.push(parsed_line);
    }
    fn merge(&mut self, other: LogDB) {
        let n = self.logs.len();
        for (path, value) in other.by_path {
            let path_map = self.by_path.entry(path).or_insert(vec![]);
            path_map.extend(value.iter().map(|x| x + n));
        }
        for (date, value) in other.by_date {
            let date_map = self.by_date.entry(date).or_insert(vec![]);
            date_map.extend(value.iter().map(|x| x + n));
        }
        for line in other.logs {
            self.logs.push(line);
        }
    }
}

#[derive(Debug, Clone)]
pub struct Config {
    pub ignore_query_params: bool,
    pub include_errors: bool,
    pub include_media: bool,
    pub include_crawlers: bool,
}

fn parse_line<'a>(line: &'a str) -> Option<ParsedLine> {
    lazy_static! {
        static ref log_line_re: Regex = Regex::new(r#"^([0-9a-f:]+?|\d{1,3}\.\d{1,3}\.\d{1,3}\.\d{1,3}) - - \[(.*?)\] "([A-Z]+) (.*?) HTTP/*.*" (\d{3}) (\d+) "(.*?)" "(.*?)"$"#).unwrap();
    }
    let captures = log_line_re.captures(line);
    match captures {
        Some(captures) => Some(ParsedLine {
            ip: String::from(captures.get(1).unwrap().as_str()),
            date: DateTime::parse_from_str(
                captures.get(2).unwrap().as_str(),
                "%d/%b/%Y:%H:%M:%S %z",
            )
            .unwrap(),
            path: String::from(captures.get(4).unwrap().as_str()),
            status: String::from(captures.get(5).unwrap().as_str())
                .parse()
                .unwrap(),
            referrer: String::from(captures.get(7).unwrap().as_str()),
            user_agent: String::from(captures.get(8).unwrap().as_str()),
        }),

        None => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_one_line() {
        let log_line = "49.206.4.211 - - [29/Oct/2018:07:35:39 -0700] \"GET / HTTP/1.1\" 200 14643 \"http://google.com\" \"Mozilla/5.0 (X11; Ubuntu; Linux x86_64; rv:64.0) Gecko/20100101 Firefox/64.0\"";
        let parsed_line = parse_line(log_line).unwrap();

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
            include_crawlers: true,
            include_media: true,
            include_errors: false,
            ignore_query_params: true,
        };
        let log_db = parse_string(String::from(log_contents), config);
        assert_eq!(log_db.by_path.keys().len(), 2);
        assert_eq!(log_db.by_path.contains_key("/rss.xml"), false);
        assert_eq!(log_db.by_path.get("/index.xml").unwrap().len(), 2);
        assert_eq!(log_db.by_path.get("/index.xml").unwrap(), &vec![1_usize, 2]);
        assert_eq!(log_db.by_path.get("/").unwrap().len(), 1);
        assert_eq!(log_db.by_path.get("/").unwrap(), &vec![0_usize]);
    }

    #[test]
    fn count_parsed_lines() {
        let log_contents = "49.206.4.211 - - [29/Oct/2018:07:35:39 -0700] \"GET / HTTP/1.1\" 200 14643 \"http://google.com\" \"Mozilla/5.0 (X11; Ubuntu; Linux x86_64; rv:64.0) Gecko/20100101 Firefox/64.0\"
54.166.138.147 - - [29/Oct/2018:07:39:20 -0700] \"GET /rss.xml HTTP/1.1\" 301 3977 \"-\" \"curl\"
54.166.138.147 - - [29/Oct/2018:07:39:20 -0700] \"GET /index.xml HTTP/1.1\" 200 42318 \"-\" \"curl\"
34.239.107.223 - - [29/Oct/2018:07:40:44 -0700] \"HEAD /rss.xml HTTP/1.1\" 301 3258 \"-\" \"Slackbot 1.0 (+https://api.slack.com/robots)\"
195.159.176.226 - - [28/Oct/2018:11:05:15 +0530] \"GET /index.xml HTTP/1.1\" 200 42318 \"-\" \"Gwene/1.0 (The gwene.org rss-to-news gateway)\"";

        let config = Config {
            include_crawlers: true,
            include_media: true,
            include_errors: false,
            ignore_query_params: true,
        };
        let parsed_content = parse_string(String::from(log_contents), config);
        let stats = compute_stats(&parsed_content);
        assert_eq!(stats[0], (2, String::from("/index.xml")));
    }

}
