use std::error::Error;
use std::fs;

pub fn run(log_path: &String) -> Result<(), Box<dyn Error>> {
    println!("Parsing logs from {}", log_path);

    let contents = fs::read_to_string(log_path).expect("Something went wrong reading the file");
    println!("With text:\n{}", contents);

    Ok(())
}
