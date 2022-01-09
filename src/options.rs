use structopt::StructOpt;

#[derive(Debug, StructOpt)]
pub struct Options {
    /// Directory to backup
    #[structopt(parse(from_os_str))]
    pub path: std::path::PathBuf,

    /// AWS region
    #[structopt(default_value = "eu-west-2", short, long)]
    pub region: String,

    /// Bucket to store data in
    #[structopt(short, long)]
    pub bucket: String,

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
    pub storage_class: String,

    /// The encryption used by the individual files
    /// Accepted values:
    /// ```
    ///  AES256
    ///  aws:kms
    /// ```
    #[structopt(default_value = "AES256", short, long)]
    pub encryption: String,
}
