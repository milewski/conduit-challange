use crate::error::CliError;
use crate::template;
use serde::Deserialize;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

const GENERATED_BINARY_NAME: &str = "generated_conduit_workflow";

#[derive(Debug)]
pub struct CompilerOptions {
    pub workflow_path: PathBuf,
    pub output_path: Option<PathBuf>,
    pub nodes_package_name: Option<String>,
    pub nodes_crate_name: Option<String>,
    pub nodes_crate_path: PathBuf,
    pub nodes_use_path: Option<String>,
}

#[derive(Debug)]
pub struct ContentCompilerOptions {
    pub output_path: PathBuf,
    pub nodes_package_name: Option<String>,
    pub nodes_crate_name: Option<String>,
    pub nodes_crate_path: PathBuf,
    pub nodes_use_path: Option<String>,
}

#[derive(Deserialize)]
struct CargoPackageSection {
    name: String,
}

#[derive(Deserialize)]
struct CargoLibrarySection {
    name: Option<String>,
}

#[derive(Deserialize)]
struct CargoManifest {
    package: Option<CargoPackageSection>,
    lib: Option<CargoLibrarySection>,
}

struct NodesConfiguration {
    package_name: String,
    crate_name: String,
    use_path: String,
}

pub fn compile_workflow(options: CompilerOptions) -> Result<PathBuf, CliError> {
    let workspace_root = resolve_workspace_root()?;
    let workflow_path = resolve_absolute_path(&options.workflow_path)?;
    validate_workflow_path(&workflow_path)?;

    let output_path = resolve_output_path(options.output_path.as_ref(), &workflow_path)?;
    let workflow_content = load_workflow(&workflow_path)?;

    compile_workflow_content(
        &workflow_content,
        ContentCompilerOptions {
            output_path,
            nodes_package_name: options.nodes_package_name,
            nodes_crate_name: options.nodes_crate_name,
            nodes_crate_path: options.nodes_crate_path,
            nodes_use_path: options.nodes_use_path,
        },
        &workspace_root,
    )
}

pub fn compile_workflow_content(
    workflow_content: &str,
    options: ContentCompilerOptions,
    workspace_root: &Path,
) -> Result<PathBuf, CliError> {
    let output_path = resolve_absolute_path(&options.output_path)?;
    let nodes_crate_path = resolve_absolute_path(&options.nodes_crate_path)?;
    validate_nodes_options(
        &nodes_crate_path,
        options.nodes_package_name.as_deref(),
        options.nodes_crate_name.as_deref(),
        options.nodes_use_path.as_deref(),
    )?;

    let conduit_crate_path = workspace_root.join("crates/conduit");
    let nodes_configuration = infer_nodes_configuration(
        &nodes_crate_path,
        options.nodes_package_name.as_deref(),
        options.nodes_crate_name.as_deref(),
        options.nodes_use_path.as_deref(),
    )?;

    let generated_project_path = create_generated_project_path(&workspace_root)?;
    let generated_manifest_path = generated_project_path.join("Cargo.toml");
    let generated_source_path = generated_project_path.join("src/main.rs");
    let generated_binary_path = generated_project_path.join(format!(
        "target/release/{}{}",
        GENERATED_BINARY_NAME,
        std::env::consts::EXE_SUFFIX
    ));

    create_directory(generated_source_path.parent().unwrap_or_else(|| Path::new(".")))?;
    write_generated_manifest(
        &generated_manifest_path,
        &conduit_crate_path,
        &nodes_configuration.crate_name,
        &nodes_configuration.package_name,
        &nodes_crate_path,
    )?;
    write_generated_source(&generated_source_path, workflow_content, &nodes_configuration.use_path)?;

    let compilation_result = (|| -> Result<(), CliError> {
        run_release_build(&generated_manifest_path)?;

        create_directory(output_path.parent().unwrap_or_else(|| Path::new(".")))?;
        copy_binary_to_output(&generated_binary_path, &output_path)?;
        set_executable_permission(&output_path)?;

        Ok(())
    })();

    let cleanup_result = cleanup_temporary_directory(&generated_project_path);

    compilation_result?;
    cleanup_result?;

    Ok(output_path)
}

fn validate_workflow_path(workflow_path: &Path) -> Result<(), CliError> {
    if !workflow_path.exists() {
        return Err(CliError::WorkflowFileMissing {
            path: workflow_path.to_path_buf(),
        });
    }

    if !workflow_path.is_file() {
        return Err(CliError::WorkflowPathInvalid {
            path: workflow_path.to_path_buf(),
        });
    }

    Ok(())
}

fn validate_nodes_options(
    nodes_crate_path: &Path,
    nodes_package_name: Option<&str>,
    nodes_crate_name: Option<&str>,
    nodes_use_path: Option<&str>,
) -> Result<(), CliError> {
    if !nodes_crate_path.exists() {
        return Err(CliError::NodesCratePathMissing {
            path: nodes_crate_path.to_path_buf(),
        });
    }

    let nodes_manifest_path = nodes_crate_path.join("Cargo.toml");

    if !nodes_manifest_path.exists() {
        return Err(CliError::NodesCrateManifestMissing {
            path: nodes_manifest_path,
        });
    }

    if let Some(nodes_package_name_value) = nodes_package_name {
        if nodes_package_name_value.trim().is_empty() {
            return Err(CliError::NodesOptionEmpty {
                option_name: "nodes_package_name",
            });
        }
    }

    validate_remaining_nodes_options(nodes_crate_name, nodes_use_path)
}

fn validate_remaining_nodes_options(
    nodes_crate_name: Option<&str>,
    nodes_use_path: Option<&str>,
) -> Result<(), CliError> {
    if let Some(nodes_crate_name_value) = nodes_crate_name {
        if nodes_crate_name_value.trim().is_empty() {
            return Err(CliError::NodesOptionEmpty {
                option_name: "nodes_crate_name",
            });
        }
    }

    if let Some(nodes_use_path_value) = nodes_use_path {
        if nodes_use_path_value.trim().is_empty() {
            return Err(CliError::NodesOptionEmpty {
                option_name: "nodes_use_path",
            });
        }
    }

    Ok(())
}

fn load_workflow(workflow_path: &Path) -> Result<String, CliError> {
    fs::read_to_string(workflow_path).map_err(|source| CliError::WorkflowReadFailed {
        path: workflow_path.to_path_buf(),
        source,
    })
}

fn infer_nodes_configuration(
    nodes_crate_path: &Path,
    nodes_package_name_override: Option<&str>,
    nodes_crate_name_override: Option<&str>,
    nodes_use_path_override: Option<&str>,
) -> Result<NodesConfiguration, CliError> {
    let nodes_manifest_path = nodes_crate_path.join("Cargo.toml");
    let cargo_manifest = load_nodes_manifest(&nodes_manifest_path)?;

    let package_name = if let Some(nodes_package_name) = nodes_package_name_override {
        nodes_package_name.to_string()
    } else {
        cargo_manifest
            .package
            .as_ref()
            .map(|package| package.name.clone())
            .ok_or_else(|| CliError::NodesPackageNameMissing {
                path: nodes_manifest_path.clone(),
            })?
    };

    let crate_name = if let Some(nodes_crate_name) = nodes_crate_name_override {
        nodes_crate_name.to_string()
    } else if let Some(library_name) = cargo_manifest
        .lib
        .as_ref()
        .and_then(|library_section| library_section.name.as_ref())
    {
        library_name.clone()
    } else {
        package_name.replace('-', "_")
    };

    let use_path = if let Some(nodes_use_path) = nodes_use_path_override {
        nodes_use_path.to_string()
    } else {
        format!("{crate_name}::*")
    };

    Ok(NodesConfiguration {
        package_name,
        crate_name,
        use_path,
    })
}

fn load_nodes_manifest(nodes_manifest_path: &Path) -> Result<CargoManifest, CliError> {
    let manifest_content =
        fs::read_to_string(nodes_manifest_path).map_err(|source| CliError::NodesManifestReadFailed {
            path: nodes_manifest_path.to_path_buf(),
            source,
        })?;

    toml::from_str(&manifest_content).map_err(|error| CliError::NodesManifestParseFailed {
        path: nodes_manifest_path.to_path_buf(),
        message: error.to_string(),
    })
}

pub fn resolve_workspace_root() -> Result<PathBuf, CliError> {
    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");

    workspace_root
        .canonicalize()
        .map_err(|source| CliError::WorkspaceRootResolutionFailed {
            path: workspace_root,
            source,
        })
}

fn resolve_absolute_path(path: &Path) -> Result<PathBuf, CliError> {
    if path.is_absolute() {
        return Ok(path.to_path_buf());
    }

    let current_directory = env::current_dir().map_err(|source| CliError::CurrentDirectoryUnavailable { source })?;

    Ok(current_directory.join(path))
}

fn resolve_output_path(output_path: Option<&PathBuf>, workflow_path: &Path) -> Result<PathBuf, CliError> {
    let Some(output_path) = output_path else {
        let inferred_name = workflow_path
            .file_stem()
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("conduit-workflow"));

        return resolve_absolute_path(&inferred_name);
    };

    if output_path.is_absolute() {
        return Ok(output_path.clone());
    }

    resolve_absolute_path(output_path)
}

fn create_generated_project_path(workspace_root: &Path) -> Result<PathBuf, CliError> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|source| CliError::SystemClockFailed { source })?
        .as_nanos();
    let process_id = std::process::id();
    let project_path = workspace_root.join(format!("target/conduit-generated/{now}-{process_id}"));

    create_directory(&project_path)?;

    Ok(project_path)
}

fn write_generated_manifest(
    manifest_path: &Path,
    conduit_crate_path: &Path,
    nodes_crate_name: &str,
    nodes_package_name: &str,
    nodes_crate_path: &Path,
) -> Result<(), CliError> {
    let content = template::render_manifest(
        conduit_crate_path,
        nodes_crate_name,
        nodes_package_name,
        nodes_crate_path,
    )?;

    write_file(manifest_path, &content)
}

fn write_generated_source(source_path: &Path, workflow_content: &str, nodes_use_path: &str) -> Result<(), CliError> {
    let content = template::render_source(workflow_content, nodes_use_path)?;

    write_file(source_path, &content)
}

fn write_file(path: &Path, content: &str) -> Result<(), CliError> {
    fs::write(path, content).map_err(|source| CliError::FileWriteFailed {
        path: path.to_path_buf(),
        source,
    })
}

fn run_release_build(manifest_path: &Path) -> Result<(), CliError> {
    let build_status = Command::new("cargo")
        .arg("build")
        .arg("--release")
        .arg("--manifest-path")
        .arg(manifest_path)
        .arg("--quiet")
        .status()
        .map_err(|source| CliError::CargoBuildSpawnFailed { source })?;

    if build_status.success() {
        return Ok(());
    }

    Err(CliError::CargoBuildFailed {
        status: build_status.to_string(),
    })
}

fn copy_binary_to_output(generated_binary_path: &Path, output_path: &Path) -> Result<(), CliError> {
    fs::copy(generated_binary_path, output_path).map_err(|source| CliError::BinaryCopyFailed {
        source_path: generated_binary_path.to_path_buf(),
        destination_path: output_path.to_path_buf(),
        source,
    })?;

    Ok(())
}

fn create_directory(path: &Path) -> Result<(), CliError> {
    fs::create_dir_all(path).map_err(|source| CliError::DirectoryCreateFailed {
        path: path.to_path_buf(),
        source,
    })
}

fn cleanup_temporary_directory(path: &Path) -> Result<(), CliError> {
    if !path.exists() {
        return Ok(());
    }

    fs::remove_dir_all(path).map_err(|source| CliError::TemporaryCleanupFailed {
        path: path.to_path_buf(),
        source,
    })
}

fn set_executable_permission(output_path: &Path) -> Result<(), CliError> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let metadata = fs::metadata(output_path).map_err(|source| CliError::OutputMetadataFailed {
            path: output_path.to_path_buf(),
            source,
        })?;
        let mut permissions = metadata.permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(output_path, permissions).map_err(|source| CliError::OutputPermissionFailed {
            path: output_path.to_path_buf(),
            source,
        })?;
    }

    Ok(())
}
