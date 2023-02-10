mod errors;
mod options;
mod s3;

use crate::errors::{BackupError, BackupResult};
use crate::options::Options as CLIopts;
use crate::s3::S3Client;

use async_recursion::async_recursion;
use aws_sdk_s3::types::ByteStream;
use log::{debug, error, info, warn};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use structopt::StructOpt;

#[tokio::main]
async fn main() {
    env_logger::init_from_env(
        env_logger::Env::default().filter_or(env_logger::DEFAULT_FILTER_ENV, "info"),
    );

    let args = CLIopts::from_args();
    let client = S3Client::new(
        &args.bucket,
        args.region,
        &args.storage_class,
        &args.encryption,
    )
    .await
    .unwrap_or_else(|err| panic!("Unable to establish S3 client: {}", err));

    let mut files_by_path = fetch_existing_objects(&client)
        .await
        .unwrap_or_else(|err| panic!("Failed to fetch objects: {}", err));

    info!("Found {} objects", files_by_path.len());

    let root =
        expand_path(args.path).unwrap_or_else(|err| panic!("Failed to read root path: {}", err));

    let second = root.clone();
    match traverse_directories(&root, &second, &mut files_by_path, &client).await {
        Ok(()) => info!("All directories synced"),
        Err(err) => error!("Failed to sync directories: {}", err),
    }
}

async fn fetch_existing_objects(client: &S3Client) -> BackupResult<HashSet<Vec<String>>> {
    let mut files_by_path = HashSet::<Vec<String>>::new();
    let mut next_token: Option<String> = None;

    loop {
        let response = client.fetch_existing_objects(next_token).await?;
        for object in response.contents().unwrap_or_default() {
            let filename = object.key().expect("No filename found!");

            let filename_pieces = split_filename(&filename);
            files_by_path.insert(filename_pieces);
        }

        next_token = response.next_continuation_token().map(|t| t.to_string());
        if response.is_truncated() {
            return Ok(files_by_path);
        }
    }
}

fn expand_path(input: PathBuf) -> BackupResult<PathBuf> {
    let expanded_path: String = shellexpand::tilde(&parse_path(input)?).to_string();
    return Ok(Path::new(&expanded_path).to_owned());
}

fn split_filename(filename: &str) -> Vec<String> {
    return filename
        .split(&['/', '\\'][..])
        .map(|s| s.to_string())
        .collect();
}

#[async_recursion]
async fn traverse_directories(
    path: &Path,
    root: &Path,
    existing_files: &mut HashSet<Vec<String>>,
    client: &S3Client,
) -> BackupResult<()> {
    // We use metadata since path::is_file() coerces an error into false
    let metadata = match fs::metadata(path) {
        Ok(m) => m,
        Err(err) => {
            warn!("Unable to read the metadata for {:?}: {}", path, err);
            return Ok(());
        }
    };

    if metadata.is_file() {
        debug!("Processing {:?}", path.file_name());
        let stripped_path = match strip_path(path, root) {
            Some(p) => p,
            None => return Ok(()),
        };
        let filename_segments = split_filename(&stripped_path);

        if existing_files.contains(&filename_segments) {
            info!("Skipping existing file: {}", stripped_path);
            return Ok(());
        }

        info!("Uploading new file: {}", stripped_path);
        existing_files.insert(filename_segments);

        let file_data = ByteStream::from_path(path).await;
        match file_data {
            Ok(data) => {
                client.upload_file(data, stripped_path.as_ref()).await?;
            }
            Err(err) => {
                error!("Failed to read file {:?}: {}", stripped_path, err);
            }
        }
        return Ok(());
    }

    debug!("Diving into new directory: {:?}", path);

    for entry in fs::read_dir(path).unwrap() {
        if let Ok(directory) = entry {
            let directory_name = parse_path(directory.path())?;

            info!("Evaluating {}", directory_name);
            traverse_directories(&directory.path(), root, existing_files, client).await?;
        }
    }

    Ok(())
}

fn parse_path(path: PathBuf) -> BackupResult<String> {
    match path.into_os_string().into_string() {
        Ok(parsed_path) => Ok(parsed_path),
        Err(err) => Err(BackupError::InvalidPath),
    }
}

fn strip_path(path: &Path, root: &Path) -> Option<String> {
    let path = match path.strip_prefix(root) {
        Ok(p) => match p.to_str() {
            Some(p) => p,
            None => {
                error!("Failed to parse path: {:?}", path);
                return None;
            }
        },
        Err(err) => {
            error!("Failed to parse path {:?}: {}", path, err);
            return None;
        }
    };

    Some(path.to_owned())
}
