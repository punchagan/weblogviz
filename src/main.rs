use std::env;
use std::fs;
use std::process;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        println!("Missing argument: path to log file");
        process::exit(1);
    }
    run(&args[1]);
}

fn run(log_path: &String) {
    println!("Parsing logs from {}", log_path);

    let contents = fs::read_to_string(log_path).expect("Something went wrong reading the file");
    println!("With text:\n{}", contents);
}
