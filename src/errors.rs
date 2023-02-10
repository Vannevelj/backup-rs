use aws_sdk_s3::{
    error::{ListObjectsV2Error, PutObjectError},
    types::{SdkError},
};
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
