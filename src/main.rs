use async_recursion::async_recursion;
use aws_sdk_s3::error::ListObjectsV2Error;
use aws_sdk_s3::model::{ServerSideEncryption, StorageClass};
use aws_sdk_s3::output::{ListObjectsV2Output, PutObjectOutput};
use aws_sdk_s3::{error::PutObjectError, ByteStream, Client, Region, SdkError};
use log::{debug, error, info, warn};
use shellexpand::{self};
use std::collections::HashSet;
use std::fs::{self};
use std::path::{Path, PathBuf};
use std::str::FromStr;
use structopt::StructOpt;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum BackupError {
    #[error("Could not parse path")]
    InvalidPath,

    #[error("Invalid storage class")]
    InvalidStorageClass,

    #[error("Invalid server side encryption")]
    InvalidServerSideEncryption,

    #[error("S3 upload failed")]
    UploadFailed(#[from] SdkError<PutObjectError>),

    #[error("Failed to retrieve data from server")]
    FileFetchFailed(#[from] SdkError<ListObjectsV2Error>),
}

pub type BackupResult<T> = Result<T, BackupError>;

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

    /// The storage class for the individual files
    /// Accepted values:
    /// ```
    ///  DEEP_ARCHIVE
    ///  GLACIER
    ///  GLACIER_IR
    ///  INTELLIGENT_TIERING
    ///  ONEZONE_IA
    ///  OUTPOSTS
    ///  REDUCED_REDUNDANCY
    ///  STANDARD
    ///  STANDARD_IA
    /// ```
    #[structopt(default_value = "DEEP_ARCHIVE", short, long)]
    storage_class: String,

    /// The encryption used by the individual files
    /// Accepted values:
    /// ```
    ///  AES256
    ///  aws:kms
    /// ```
    #[structopt(default_value = "AES256", short, long)]
    encryption: String,
}

#[tokio::main]
async fn main() {
    env_logger::init_from_env(
        env_logger::Env::default().filter_or(env_logger::DEFAULT_FILTER_ENV, "info"),
    );

    let args = Options::from_args();
    let client = match S3Client::new(
        &args.bucket,
        args.region,
        &args.storage_class,
        &args.encryption,
    )
    .await
    {
        Ok(c) => c,
        Err(err) => panic!("Unable to establish S3 client: {}", err),
    };

    let mut files_by_path = match fetch_existing_objects(&args.bucket, &client).await {
        Ok(files) => files,
        Err(error) => panic!("Failed to fetch objects: {}", error),
    };

    info!("Found {} objects", files_by_path.len());

    let root = match expand_path(args.path) {
        Ok(p) => p,
        Err(error) => panic!("Failed to read root path: {}", error),
    };

    let second = root.clone();
    match traverse_directories(
        &root,
        &second,
        &mut files_by_path,
        &client,
        &client.bucket,
        &client.storage_class,
        &client.encryption,
    )
    .await
    {
        Ok(()) => info!("All directories synced"),
        Err(err) => error!("Failed to sync directories: {}", err),
    }
}

async fn fetch_existing_objects(
    bucket: &str,
    client: &S3Client,
) -> Result<HashSet<Vec<String>>, Box<dyn std::error::Error>> {
    let mut files_by_path = HashSet::<Vec<String>>::new();
    let mut next_token: Option<String> = None;

    loop {
        let response = client.fetch_existing_objects(next_token).await?;
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

fn expand_path(input: std::path::PathBuf) -> BackupResult<PathBuf> {
    let expanded_path: String = shellexpand::tilde::<String>(&parse_path(input)?).to_string();
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
    path: &std::path::Path,
    root: &std::path::Path,
    existing_files: &mut HashSet<Vec<String>>,
    client: &S3Client,
    bucket: &str,
    storage_class: &StorageClass,
    sse: &ServerSideEncryption,
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
        let stripped_path = match path.strip_prefix(root) {
            Ok(p) => match p.to_str() {
                Some(p) => p,
                None => {
                    error!("Failed to parse path: {:?}", path);
                    return Ok(());
                }
            },
            Err(err) => {
                error!("Failed to parse path {:?}: {}", path, err);
                return Ok(());
            }
        };
        
        let filename_segments = split_filename(stripped_path);

        if existing_files.contains(&filename_segments) {
            info!("Skipping existing file: {}", stripped_path);
            return Ok(());
        }

        info!("Uploading new file: {}", stripped_path);
        existing_files.insert(filename_segments);

        let file_data = ByteStream::from_path(path).await;
        match file_data {
            Ok(data) => {
                client.upload_file(data, stripped_path).await?;
            }
            Err(err) => {
                error!("Failed to read file {:?}: {}", stripped_path, err);
            }
        }
        return Ok(());
    }

    debug!("Diving into new directory: {:?}", path);

    for entry in fs::read_dir(path).unwrap() {
        let directory = entry.unwrap();
        let directory_name = parse_path(directory.path())?;

        info!("Evaluating {}", directory_name);
        traverse_directories(
            &directory.path(),
            root,
            existing_files,
            &client,
            bucket,
            storage_class,
            sse,
        )
        .await
        .unwrap();
    }

    Ok(())
}

fn parse_path(path: PathBuf) -> BackupResult<String> {
    return match path.into_os_string().into_string() {
        Ok(parsed_path) => Ok(parsed_path),
        Err(err) => Err(BackupError::InvalidPath),
    };
}

pub struct S3Client {
    s3_client: Client,
    bucket: String,
    storage_class: StorageClass,
    encryption: ServerSideEncryption,
}

impl S3Client {
    pub async fn new(
        bucket: &str,
        region: String,
        storage_class: &str,
        sse: &str,
    ) -> BackupResult<S3Client> {
        let region = Region::new(region);
        let aws_config = aws_config::from_env().region(region).load().await;
        let client = Client::new(&aws_config);

        let storage_class = match StorageClass::from_str(storage_class) {
            Ok(class) => class,
            Err(err) => return Err(BackupError::InvalidStorageClass),
        };

        let sse = match ServerSideEncryption::from_str(sse) {
            Ok(enc) => enc,
            Err(err) => return Err(BackupError::InvalidStorageClass),
        };

        return Ok(S3Client {
            s3_client: client,
            bucket: bucket.to_owned(),
            storage_class,
            encryption: sse,
        });
    }

    pub async fn upload_file(&self, data: ByteStream, key: &str) -> BackupResult<PutObjectOutput> {
        let upload_response = self
            .s3_client
            .put_object()
            .bucket(&self.bucket)
            .key(key.replace("\\", "/"))
            .body(data)
            .set_storage_class(Some(self.storage_class.to_owned()))
            .server_side_encryption(self.encryption.to_owned())
            .send()
            .await;

        match upload_response {
            Ok(output) => Ok(output),
            Err(err) => Err(BackupError::UploadFailed(err)),
        }
    }

    pub async fn fetch_existing_objects(
        &self,
        continuation_token: Option<String>,
    ) -> BackupResult<ListObjectsV2Output> {
        let response = self
            .s3_client
            .list_objects_v2()
            .bucket(&self.bucket)
            .set_continuation_token(continuation_token.or(None))
            .send()
            .await;

        match response {
            Ok(output) => Ok(output),
            Err(err) => Err(BackupError::FileFetchFailed(err)),
        }
    }
}
