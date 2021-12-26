use aws_sdk_s3::{Client, Region};
use std::fs::{self};
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
struct Options {
    #[structopt(parse(from_os_str))]
    path: std::path::PathBuf,

    #[structopt(default_value = "eu-west-2", long)]
    region: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Options::from_args();
    //println!("{:?}", args);
    let region = Region::new(args.region);
    let aws_config = aws_config::from_env().region(region).load().await;
    let client = Client::new(&aws_config);

    let response = client.list_buckets().send().await?;
    let buckets = response.buckets().unwrap_or_default();

    for bucket in buckets {
        println!("{:?}", bucket);
    }

    return sync_directories(args.path);
}

fn sync_directories(path: std::path::PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    for entry in fs::read_dir(path)? {
        let directory = match entry {
            Ok(content) => content,
            Err(error) => panic!("Invalid directory! {}", error),
        };
        println!("{:?}", directory);
    }

    Ok(())
}
