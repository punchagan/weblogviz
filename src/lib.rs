#[macro_use]
extern crate lazy_static;
extern crate regex;

use std::error::Error;
use std::fs;

use regex::Regex;

pub fn run(log_path: &String) -> Result<(), Box<dyn Error>> {
    println!("Parsing logs from {}", log_path);

    let contents = fs::read_to_string(log_path).expect("Something went wrong reading the file");
    for line in contents.lines() {
        let parsed = parse_line(line);
        if parsed.status == 200 {
            println!(
                "{} from {} by ({}: {})",
                parsed.path, parsed.referrer, parsed.user_agent, parsed.ip
            );
        }
    }

    Ok(())
}

struct ParsedLine {
    ip: String,
    path: String,
    status: i32,
    referrer: String,
    user_agent: String,
}

fn parse_line<'a>(line: &'a str) -> ParsedLine {
    lazy_static! {
        static ref RE: Regex = Regex::new(r#"^(\d{1,3}\.\d{1,3}\.\d{1,3}\.\d{1,3}) - - (.*?) "([A-Z]+) (.*?) HTTP/*.*" (\d{3}) (\d+) "(.*?)" "(.*?)"$"#).unwrap();
    }
    let captures = RE.captures(line).unwrap();

    ParsedLine {
        ip: String::from(captures.get(1).unwrap().as_str()),
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
        assert_eq!("/", parse_line(log_line).path);
        assert_eq!(200, parse_line(log_line).status);
        assert_eq!("http://google.com", parse_line(log_line).referrer);
        assert_eq!(
            "Mozilla/5.0 (X11; Ubuntu; Linux x86_64; rv:64.0) Gecko/20100101 Firefox/64.0",
            parse_line(log_line).user_agent
        );
    }
}
