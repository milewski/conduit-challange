use crate::error::CliError;
use handlebars::Handlebars;
use serde::Serialize;
use std::path::Path;

const GENERATED_MANIFEST_TEMPLATE: &str = include_str!("templates/generated_Cargo.toml.hbs");
const GENERATED_SOURCE_TEMPLATE: &str = include_str!("templates/generated_main.rs.hbs");

#[derive(Serialize)]
pub struct GeneratedManifestContext {
    pub conduit_crate_path: String,
    pub nodes_crate_name: String,
    pub nodes_package_name: String,
    pub nodes_crate_path: String,
}

#[derive(Serialize)]
pub struct GeneratedSourceContext {
    pub workflow_literal: String,
    pub nodes_use_path: String,
}

pub fn render_manifest(
    conduit_crate_path: &Path,
    nodes_crate_name: &str,
    nodes_package_name: &str,
    nodes_crate_path: &Path,
) -> Result<String, CliError> {
    let context = GeneratedManifestContext {
        conduit_crate_path: conduit_crate_path.display().to_string(),
        nodes_crate_name: nodes_crate_name.to_string(),
        nodes_package_name: nodes_package_name.to_string(),
        nodes_crate_path: nodes_crate_path.display().to_string(),
    };

    render_template("generated_Cargo.toml.hbs", GENERATED_MANIFEST_TEMPLATE, &context)
}

pub fn render_source(workflow_content: &str, nodes_use_path: &str) -> Result<String, CliError> {
    let context = GeneratedSourceContext {
        workflow_literal: format!("{workflow_content:?}"),
        nodes_use_path: nodes_use_path.to_string(),
    };

    render_template("generated_main.rs.hbs", GENERATED_SOURCE_TEMPLATE, &context)
}

fn render_template<T: Serialize>(
    template_name: &'static str,
    template_content: &str,
    context: &T,
) -> Result<String, CliError> {
    let mut template_engine = Handlebars::new();
    template_engine.register_escape_fn(handlebars::no_escape);

    template_engine
        .register_template_string(template_name, template_content)
        .map_err(|error| CliError::TemplateRenderFailed {
            file_name: template_name,
            message: error.to_string(),
        })?;

    template_engine
        .render(template_name, context)
        .map_err(|error| CliError::TemplateRenderFailed {
            file_name: template_name,
            message: error.to_string(),
        })
}
