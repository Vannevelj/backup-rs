use structopt::StructOpt;
use std::fs::{self};

#[derive(Debug, StructOpt)]
struct Options {
    #[structopt(parse(from_os_str))]
    path: std::path::PathBuf
}

fn main() -> Result<(), Box<dyn std::error::Error>>{
    let args = Options::from_args();
    println!("{:?}", args);

    for entry in fs::read_dir(args.path)? {
        let directory = match entry {
            Ok(content) => { content },
            Err(error) => panic!("Invalid directory! {}", error)
        };
        println!("{:?}", directory);
    }

    Ok(())
}
