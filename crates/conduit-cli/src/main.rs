mod cli;
mod compiler;
mod error;
mod template;

use clap::Parser;
use cli::ConduitCli;

fn main() {
    let command_line = ConduitCli::parse();

    match compiler::compile_workflow(command_line.into_options()) {
        Ok(output_path) => println!("Generated executable: {}", output_path.display()),
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(1);
        }
    }
}
