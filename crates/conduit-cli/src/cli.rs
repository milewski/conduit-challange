use crate::compiler::CompilerOptions;
use clap::Parser;
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(name = "conduit", about = "Compile a .conduit workflow into a standalone executable")]
pub struct ConduitCli {
    #[arg(value_name = "WORKFLOW_FILE")]
    workflow_path: PathBuf,

    #[arg(short, long = "output", value_name = "OUTPUT_PATH")]
    output_path: Option<PathBuf>,

    #[arg(long, default_value = "conduit-example")]
    nodes_package_name: String,

    #[arg(long, default_value = "example")]
    nodes_crate_name: String,

    #[arg(long, default_value = "examples/conduit-example", value_name = "NODES_CRATE_PATH")]
    nodes_crate_path: PathBuf,

    #[arg(long, default_value = "example::nodes::*", value_name = "NODES_USE_PATH")]
    nodes_use_path: String,
}

impl ConduitCli {
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
