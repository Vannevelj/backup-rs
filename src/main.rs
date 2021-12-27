use async_recursion::async_recursion;
use aws_sdk_s3::{ByteStream, Client, Region};
use log::{error, info};
use shellexpand::{self};
use std::collections::HashSet;
use std::fs::{self};
use std::io::{Error, ErrorKind};
use std::path::Path;
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
async fn main() {
    env_logger::init_from_env(
        env_logger::Env::default().filter_or(env_logger::DEFAULT_FILTER_ENV, "info"),
    );

    let args = Options::from_args();
    let region = Region::new(args.region);
    let aws_config = aws_config::from_env().region(region).load().await;
    let client = Client::new(&aws_config);

    let mut files_by_path = match fetch_existing_objects(&args.bucket, &client).await {
        Ok(files) => files,
        Err(error) => panic!("Failed to fetch objects: {}", error),
    };

    info!("Found {} objects", files_by_path.len());

    let root = expand_path(args.path);
    let second = root.clone();
    match traverse_directories(&root, &second, &mut files_by_path, &client, &args.bucket).await {
        Ok(()) => info!("All directories synced"),
        Err(err) => error!("Failed to sync directories: {}", err),
    }
}

async fn fetch_existing_objects(
    bucket: &String,
    aws_client: &Client,
) -> Result<HashSet<Vec<String>>, Box<dyn std::error::Error>> {
    let mut files_by_path = HashSet::<Vec<String>>::new();
    let mut next_token: Option<String> = None;

    loop {
        let response = aws_client
            .list_objects_v2()
            .bucket(bucket)
            .set_continuation_token(next_token.take())
            .send()
            .await?;
        for object in response.contents().unwrap_or_default() {
            let filename = match object.key() {
                Some(name) => name,
                None => panic!("No filename found!"),
            };

            let filename_pieces = split_filename(filename);
            files_by_path.insert(filename_pieces);
        }

        next_token = response.next_continuation_token().map(|t| t.to_string());
        if !response.is_truncated() {
            break;
        }
    }

    Ok(files_by_path)
}

fn expand_path(input: std::path::PathBuf) -> std::path::PathBuf {
    let expanded_path: String =
        shellexpand::tilde::<String>(&input.into_os_string().into_string().unwrap()).to_string();
    return Path::new(&expanded_path).to_owned();
}

fn split_filename(filename: &str) -> Vec<String> {
    return filename.split("/").map(|s| s.to_string()).collect();
}

#[async_recursion]
async fn traverse_directories(
    path: &std::path::PathBuf,
    root: &std::path::PathBuf,
    existing_files: &mut HashSet<Vec<String>>,
    aws_client: &Client,
    bucket: &String,
) -> Result<(), Box<dyn std::error::Error>> {
    if path.is_file() {
        let stripped_path = path
            .strip_prefix(root)?
            .file_name()
            .unwrap()
            .to_str()
            .unwrap();
        let filename_segments = split_filename(stripped_path);
        if !existing_files.contains(&filename_segments) {
            info!("Uploading new file: {}", stripped_path);
            existing_files.insert(filename_segments);

            let file_data = ByteStream::from_path(path).await;
            match file_data {
                Ok(data) => {
                    let upload_response = aws_client
                        .put_object()
                        .bucket(bucket)
                        .key(stripped_path)
                        .body(data)
                        .send()
                        .await?;
                }
                Err(err) => {
                    error!("Failed to upload file {:?}: {}", stripped_path, err);
                }
            }

            // upload file
        }
        return Ok(());
    }

    for entry in fs::read_dir(path)? {
        let directory = entry?;
        let directory_name = match directory.path().into_os_string().into_string() {
            Ok(name) => name,
            Err(error) => {
                return Err(Error::new(
                    ErrorKind::Other,
                    format!("Could not parse path: {:?}", error),
                )
                .into())
            }
        };

        info!("Evaluating {}", directory_name);
        traverse_directories(&directory.path(), root, existing_files, aws_client, bucket).await?;
    }

    Ok(())
}
