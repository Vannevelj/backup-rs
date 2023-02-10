use crate::errors::{BackupError, BackupResult};
use aws_sdk_s3::model::{ServerSideEncryption, StorageClass};
use aws_sdk_s3::output::{ListObjectsV2Output, PutObjectOutput};
use aws_sdk_s3::{types::{ByteStream}, Client, Region};
use std::str::FromStr;

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
            Err(err) => return Err(BackupError::InvalidServerSideEncryption),
        };

        Ok(S3Client {
            s3_client: client,
            bucket: bucket.to_owned(),
            storage_class,
            encryption: sse,
        })
    }

    pub async fn upload_file(&self, data: ByteStream, key: &str) -> BackupResult<PutObjectOutput> {
        self.s3_client
            .put_object()
            .bucket(&self.bucket)
            .key(key.replace("\\", "/"))
            .body(data)
            .set_storage_class(Some(self.storage_class.to_owned()))
            .server_side_encryption(self.encryption.to_owned())
            .send()
            .await
            .map_err(BackupError::UploadFailed)
    }

    pub async fn fetch_existing_objects(
        &self,
        continuation_token: Option<String>,
    ) -> BackupResult<ListObjectsV2Output> {
        self.s3_client
            .list_objects_v2()
            .bucket(&self.bucket)
            .set_continuation_token(continuation_token.or(None))
            .send()
            .await
            .map_err(BackupError::FileFetchFailed)
    }
}
