use crate::compiler::CompilerOptions;
use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(name = "conduit", about = "Compile a .conduit workflow into a standalone executable")]
pub struct ConduitCli {
    #[command(subcommand)]
    pub command: ConduitCommand,
}

#[derive(Debug, Subcommand)]
pub enum ConduitCommand {
    Compile(CompileCommand),
    Install(InstallCommand),
}

#[derive(Debug, Parser)]
pub struct CompileCommand {
    #[arg(value_name = "WORKFLOW_FILE")]
    pub workflow_path: PathBuf,

    #[arg(short, long = "output", value_name = "OUTPUT_PATH")]
    pub output_path: Option<PathBuf>,

    #[arg(long, value_name = "NODES_PACKAGE_NAME")]
    pub nodes_package_name: Option<String>,

    #[arg(long, value_name = "NODES_CRATE_NAME")]
    pub nodes_crate_name: Option<String>,

    #[arg(long, default_value = "examples/conduit-example", value_name = "NODES_CRATE_PATH")]
    pub nodes_crate_path: PathBuf,

    #[arg(long, value_name = "NODES_USE_PATH")]
    pub nodes_use_path: Option<String>,
}

#[derive(Debug, Parser)]
pub struct InstallCommand {
    #[arg(value_name = "PATH_TO_THE_SOURCE_CODE")]
    pub source_code_path: PathBuf,
}

impl CompileCommand {
    pub fn into_options(self) -> CompilerOptions {
        CompilerOptions {
            workflow_path: self.workflow_path,
            output_path: self.output_path,
            nodes_package_name: self.nodes_package_name,
            nodes_crate_name: self.nodes_crate_name,
            nodes_crate_path: self.nodes_crate_path,
            nodes_use_path: self.nodes_use_path,
        }
    }
}
