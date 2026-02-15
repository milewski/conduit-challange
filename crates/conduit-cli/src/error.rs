use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum CliError {
    #[error("Workflow file does not exist: {path}")]
    WorkflowFileMissing { path: PathBuf },

    #[error("Invalid workflow path: {path}")]
    WorkflowPathInvalid { path: PathBuf },

    #[error("Nodes crate path does not exist: {path}")]
    NodesCratePathMissing { path: PathBuf },

    #[error("Nodes crate path must contain Cargo.toml: {path}")]
    NodesCrateManifestMissing { path: PathBuf },

    #[error("Failed to read workflow '{path}': {source}")]
    WorkflowReadFailed { path: PathBuf, source: std::io::Error },

    #[error("Failed to render generated file '{file_name}': {message}")]
    TemplateRenderFailed { file_name: &'static str, message: String },

    #[error("Failed to write generated file '{path}': {source}")]
    FileWriteFailed { path: PathBuf, source: std::io::Error },

    #[error("Failed to create directory '{path}': {source}")]
    DirectoryCreateFailed { path: PathBuf, source: std::io::Error },

    #[error("Failed to execute cargo build: {source}")]
    CargoBuildSpawnFailed { source: std::io::Error },

    #[error("Generated project build failed with status {status}")]
    CargoBuildFailed { status: String },

    #[error("Failed to copy generated binary from '{source_path}' to '{destination_path}': {source}")]
    BinaryCopyFailed {
        source_path: PathBuf,
        destination_path: PathBuf,
        source: std::io::Error,
    },

    #[error("Failed to read metadata for output binary '{path}': {source}")]
    OutputMetadataFailed { path: PathBuf, source: std::io::Error },

    #[error("Failed to set executable permissions for '{path}': {source}")]
    OutputPermissionFailed { path: PathBuf, source: std::io::Error },

    #[error("Failed to clean temporary directory '{path}': {source}")]
    TemporaryCleanupFailed { path: PathBuf, source: std::io::Error },

    #[error("Failed to resolve current directory: {source}")]
    CurrentDirectoryUnavailable { source: std::io::Error },

    #[error("Failed to resolve workspace root '{path}': {source}")]
    WorkspaceRootResolutionFailed { path: PathBuf, source: std::io::Error },

    #[error("System clock error: {source}")]
    SystemClockFailed { source: std::time::SystemTimeError },
}
