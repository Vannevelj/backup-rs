use structopt::StructOpt;
use std::fs::{self};
use aws_sdk_s3::Region;

#[derive(Debug, StructOpt)]
struct Options {
    #[structopt(parse(from_os_str))]
    path: std::path::PathBuf,

    #[structopt(default_value = "eu-west", long)]
    region: String
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>>{
    let args = Options::from_args();
    //println!("{:?}", args);
    let region = Region::new(args.region);
    let aws_config = aws_config::from_env().region(region).load().await;

    for entry in fs::read_dir(args.path)? {
        let directory = match entry {
            Ok(content) => { content },
            Err(error) => panic!("Invalid directory! {}", error)
        };
        println!("{:?}", directory);
    }

    Ok(())
}
