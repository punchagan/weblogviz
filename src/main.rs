#[macro_use]
extern crate clap;
extern crate weblogviz;

use std::process;

fn main() {
    let yaml = load_yaml!("cli.yml");
    let m = clap::App::from_yaml(yaml).get_matches();
    if let Some(path) = m.value_of("INPUT") {
        if let Err(e) = weblogviz::run(&String::from(path)) {
            println!("Application error: {}", e);
            process::exit(1);
        }
    } else {
        println!("Missing argument: path to log file");
        process::exit(1);
    }
}
