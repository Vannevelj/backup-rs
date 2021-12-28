fn main() {
    // Custom build script is required to embed a manifest which enables long path support on Windows
    embed_resource::compile("backup-rs.rc");
}