#[macro_use]
extern crate clap;
extern crate weblogviz;

use std::process;

fn main() {
    let yaml = load_yaml!("cli.yml");
    let m = clap::App::from_yaml(yaml).get_matches();
    let num_log: Result<usize, _> = String::from(m.value_of("n").unwrap_or("10")).parse();
    let paths: Vec<String> = m
        .values_of("INPUT")
        .unwrap()
        .map(|e| String::from(e))
        .collect();
    let config = weblogviz::Config {
        include_errors: m.is_present("include-errors"),
        include_media: m.is_present("include-media"),
        ignore_query_params: m.is_present("ignore-query-params"),
    };
    match num_log {
        Err(_) => {
            println!("Number of lines needs to be a number");
            process::exit(1);
        }
        Ok(_) => {}
    }

    if let Err(e) = weblogviz::run(paths, num_log.unwrap(), config) {
        println!("Application error: {}", e);
        process::exit(1);
    }
}
