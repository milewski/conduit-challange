use crate::compiler::{ContentCompilerOptions, compile_workflow_content, resolve_workspace_root};
use crate::error::CliError;
use std::fs;
use std::path::{Path, PathBuf};

pub fn install_hyperskill(source_code_path: PathBuf) -> Result<PathBuf, CliError> {
    let resolved_source_code_path = resolve_absolute_path(&source_code_path)?;

    if !resolved_source_code_path.is_dir() {
        return Err(CliError::SourceCodePathInvalid {
            path: resolved_source_code_path,
        });
    }

    let hyperskill_name = infer_hyperskill_name(&resolved_source_code_path)?;
    let source_markdown_path = resolve_hyperskill_markdown_path(&resolved_source_code_path)?;
    let install_directory = resolve_absolute_path(Path::new("hyperskills"))?.join(&hyperskill_name);
    let target_markdown_path = install_directory.join("hyperskill.md");
    let markdown_content =
        fs::read_to_string(&source_markdown_path).map_err(|source| CliError::WorkflowReadFailed {
            path: source_markdown_path.clone(),
            source,
        })?;

    let hyperskill_workflows = extract_hyperskill_blocks(&markdown_content);

    if hyperskill_workflows.is_empty() {
        return Err(CliError::HyperskillBlocksMissing {
            path: source_markdown_path,
        });
    }

    fs::create_dir_all(&install_directory).map_err(|source| CliError::DirectoryCreateFailed {
        path: install_directory.clone(),
        source,
    })?;

    fs::copy(&source_markdown_path, &target_markdown_path).map_err(|source| CliError::FileWriteFailed {
        path: target_markdown_path,
        source,
    })?;

    let workspace_root = resolve_workspace_root()?;

    for (index, workflow_content) in hyperskill_workflows.iter().enumerate() {
        let binary_name = if index == 0 {
            "compiled_hyperskill".to_string()
        } else {
            format!("compiled_hyperskill_{}", index + 1)
        };

        let output_path = install_directory.join(binary_name);

        compile_workflow_content(
            workflow_content,
            ContentCompilerOptions {
                output_path,
                nodes_package_name: None,
                nodes_crate_name: None,
                nodes_crate_path: resolved_source_code_path.clone(),
                nodes_use_path: None,
            },
            &workspace_root,
        )?;
    }

    Ok(install_directory)
}

fn resolve_hyperskill_markdown_path(source_code_path: &Path) -> Result<PathBuf, CliError> {
    let direct_markdown_path = source_code_path.join("hyperskill.md");

    if direct_markdown_path.exists() {
        return Ok(direct_markdown_path);
    }

    let parent_markdown_path = source_code_path
        .parent()
        .map(|parent_path| parent_path.join("hyperskill.md"))
        .ok_or_else(|| CliError::HyperskillMarkdownMissing {
            path: direct_markdown_path.clone(),
        })?;

    if parent_markdown_path.exists() {
        return Ok(parent_markdown_path);
    }

    Err(CliError::HyperskillMarkdownMissing {
        path: direct_markdown_path,
    })
}

fn infer_hyperskill_name(source_code_path: &Path) -> Result<String, CliError> {
    let source_directory_name = source_code_path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| CliError::SourceCodePathInvalid {
            path: source_code_path.to_path_buf(),
        })?;

    if source_directory_name == "source" {
        let parent_name = source_code_path
            .parent()
            .and_then(|parent_path| parent_path.file_name())
            .and_then(|name| name.to_str())
            .ok_or_else(|| CliError::SourceCodePathInvalid {
                path: source_code_path.to_path_buf(),
            })?;

        return Ok(parent_name.to_string());
    }

    Ok(source_directory_name.to_string())
}

fn extract_hyperskill_blocks(markdown_content: &str) -> Vec<String> {
    let mut extracted_blocks = Vec::new();
    let mut current_block_lines = Vec::new();
    let mut inside_hyperskill_block = false;

    for line in markdown_content.lines() {
        let trimmed_line = line.trim_start();

        if !inside_hyperskill_block && trimmed_line.starts_with("```hyperskill") {
            inside_hyperskill_block = true;
            current_block_lines.clear();
            continue;
        }

        if inside_hyperskill_block && trimmed_line.starts_with("```") {
            let block_content = current_block_lines.join("\n");

            if !block_content.trim().is_empty() {
                extracted_blocks.push(block_content);
            }

            inside_hyperskill_block = false;
            current_block_lines.clear();
            continue;
        }

        if inside_hyperskill_block {
            current_block_lines.push(line.to_string());
        }
    }

    extracted_blocks
}

fn resolve_absolute_path(path: &Path) -> Result<PathBuf, CliError> {
    if path.is_absolute() {
        return Ok(path.to_path_buf());
    }

    let current_directory =
        std::env::current_dir().map_err(|source| CliError::CurrentDirectoryUnavailable { source })?;

    Ok(current_directory.join(path))
}
