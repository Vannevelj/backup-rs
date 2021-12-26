use aws_sdk_s3::{Client, Region};
use std::collections::HashSet;
use std::fs::{self};
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
struct Options {
    /// Directory to backup
    #[structopt(parse(from_os_str))]
    path: std::path::PathBuf,

    /// AWS region
    #[structopt(default_value = "eu-west-2", short, long)]
    region: String,

    /// Bucket to store data in
    #[structopt(short, long)]
    bucket: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Options::from_args();
    let region = Region::new(args.region);
    let aws_config = aws_config::from_env().region(region).load().await;
    let client = Client::new(&aws_config);

    let files_by_path = match fetch_existing_objects(&args.bucket, client).await {
        Ok(files) => files,
        Err(error) => panic!("Failed to fetch objects: {}", error),
    };

    let count = files_by_path.len();

    for i in files_by_path {
        println!("{:?}", i);
    }

    println!("Found {} objects", count);

    return sync_directories(args.path);
}

async fn fetch_existing_objects(
    bucket: &String,
    aws_client: Client,
) -> Result<HashSet<Vec<String>>, Box<dyn std::error::Error>> {
    let mut files_by_path = HashSet::<Vec<String>>::new();
    let mut next_token: Option<String> = None;

    loop {
        let token = next_token.take();
        println!("Current token: {:?}", token);

        let response = aws_client
            .list_objects_v2()
            .bucket(bucket)
            .set_continuation_token(token)
            .send()
            .await?;
        for object in response.contents().unwrap_or_default() {
            println!("{:?}", object);
            let filename = match object.key() {
                Some(name) => name,
                None => panic!("No filename found!"),
            };

            let filename_pieces = filename.split("/").map(|s| s.to_string()).collect();
            files_by_path.insert(filename_pieces);
        }

        next_token = response.next_continuation_token().map(|t| t.to_string());
        if !response.is_truncated() {
            break;
        }
    }

    Ok(files_by_path)
}

fn sync_directories(path: std::path::PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    for entry in fs::read_dir(path)? {
        let directory = match entry {
            Ok(content) => content,
            Err(error) => panic!("Invalid directory! {}", error),
        };

        let directory_name = match directory.path().into_os_string().into_string() {
            Ok(name) => name,
            Err(error) => panic!("Invalid directory name! {:?}", error),
        };

        //println!("Evaluating {}", directory_name);
    }

    Ok(())
}
