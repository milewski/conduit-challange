mod cli;
mod compiler;
mod error;
mod installer;
mod template;

use clap::Parser;
use cli::{ConduitCli, ConduitCommand};

fn main() {
    let command_line = ConduitCli::parse();

    let command_result = match command_line.command {
        ConduitCommand::Compile(compile_command) => compiler::compile_workflow(compile_command.into_options())
            .map(|output_path| ("Generated executable", output_path)),
        ConduitCommand::Install(install_command) => installer::install_hyperskill(install_command.source_code_path)
            .map(|install_path| ("Installed hyperskill at", install_path)),
    };

    match command_result {
        Ok((message, output_path)) => println!("{message}: {}", output_path.display()),
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(1);
        }
    }
}
