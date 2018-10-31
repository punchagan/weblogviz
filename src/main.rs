#[macro_use]
extern crate clap;
extern crate weblogviz;

use std::process;

fn main() {
    let yaml = load_yaml!("cli.yml");
    let m = clap::App::from_yaml(yaml).get_matches();
    let num_log: Result<usize, _> = String::from(m.value_of("n").unwrap_or("10")).parse();
    match num_log {
        Err(_) => {
            println!("Number of lines needs to be a number");
            process::exit(1);
        }
        Ok(_) => {}
    }
    if let Some(path) = m.value_of("INPUT") {
        if let Err(e) = weblogviz::run(&String::from(path), num_log.unwrap()) {
            println!("Application error: {}", e);
            process::exit(1);
        }
    } else {
        println!("Missing argument: path to log file");
        process::exit(1);
    }
}
