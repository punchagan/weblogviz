extern crate weblogviz;

use std::env;
use std::process;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        println!("Missing argument: path to log file");
        process::exit(1);
    }

    if let Err(e) = weblogviz::run(&args[1]) {
        println!("Application error: {}", e);
        process::exit(1);
    }
}
