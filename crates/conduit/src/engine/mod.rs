use crate::dsl::parser::{
    Direction, Expression, Identifier, NodeInstruct, NodeParser, Operation, PIPELINE_RESULT_ID, ParsedWorkflow,
    StringPart, Value,
};
use crate::node::FromSharedValue;
use crate::node::SharedValue;
use crate::registry::{NodeRegistry, Payload};
use crate::traits::NodeOutput;
use petgraph::dot::{Config, Dot};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::Arc;

mod callback_execution;
mod graph;

use graph::{build_dependency_graph, compute_execution_levels};

pub struct Engine {
    registry: Arc<NodeRegistry>,
    runtime: tokio::runtime::Runtime,
}

impl Engine {
    pub fn new() -> Self {
        let mut registry = NodeRegistry::new();
        registry.register_all();

        Self {
            registry: Arc::new(registry),
            runtime: tokio::runtime::Runtime::new().unwrap(),
        }
    }

    pub fn run_pipeline_blocking<I: NodeOutput, T: FromSharedValue>(
        &mut self,
        workflow: &str,
        input: I,
    ) -> Result<T, crate::node::NodeError> {
        self.runtime.block_on(self.run_pipeline_async(workflow, input))
    }

    pub fn validate_modules(&self, nodes: &BTreeMap<Identifier, NodeInstruct>) -> Result<(), crate::node::NodeError> {
        for (_, instruct) in nodes {
            let module_name = &instruct.module;
            if module_name == "_" || module_name == "__input__" {
                continue;
            }

            if !self.registry.has(module_name) {
                return Err(crate::node::NodeError::ModuleValidationError(format!(
                    "Unknown module '{}' in node '{}'",
                    module_name, instruct.identifier
                )));
            }

            let module_instance = self
                .registry
                .create(module_name, Default::default())
                .ok_or_else(|| crate::node::NodeError::ModuleNotFound(module_name.clone()))?;
            let input_fields: HashSet<String> = module_instance
                .input_fields()
                .into_iter()
                .map(|field_name| field_name.to_string())
                .collect();

            for (property_name, value) in &instruct.inputs {
                if !is_input_direction(value) {
                    continue;
                }

                if !input_fields.contains(property_name) {
                    return Err(crate::node::NodeError::ModuleValidationError(format!(
                        "Unknown input '{}' for module '{}' in node '{}'",
                        property_name, module_name, instruct.identifier
                    )));
                }
            }
        }
        Ok(())
    }

    pub fn generate_dot_graph(&self, workflow: &str) -> Result<String, crate::node::NodeError> {
        let ParsedWorkflow {
            nodes,
            inputs,
            event_handlers,
            event_callback_nodes: _,
            sequential_edges,
        } = NodeParser::parse(workflow).map_err(|error| crate::node::NodeError::ParseError(format!("{}", error)))?;

        let (graph, _) = build_dependency_graph(&nodes, &event_handlers, &sequential_edges);

        let get_node_attributes = |_, (_, id): (_, &Identifier)| {
            if let Some(instruct) = nodes.get(id) {
                let module_name = if id == PIPELINE_RESULT_ID {
                    "Result"
                } else {
                    &instruct.module
                };
                let mut label = format!("{} ({})", module_name, id);
                if !instruct.inputs.is_empty() {
                    label.push_str("\\n");
                    let props: Vec<String> = instruct
                        .inputs
                        .iter()
                        .map(|(k, v)| {
                            let val_str = match v {
                                Value::String { parts, .. } => {
                                    let s: String = parts
                                        .iter()
                                        .map(|p| match p {
                                            StringPart::Literal(l) => l.clone(),
                                            StringPart::Interpolation(_) => "${...}".to_string(),
                                        })
                                        .collect();
                                    format!("\\\"{}\\\"", s.replace("\"", "\\\""))
                                }
                                Value::Numeric { value, .. } => value.clone(),
                                Value::Boolean { value, .. } => value.to_string(),
                                Value::Relation {
                                    identifier, property, ..
                                } => {
                                    if let Some(target) = nodes.get(identifier) {
                                        format!("{} ({}) :: {}", target.module, identifier, property)
                                    } else if inputs.contains_key(identifier) {
                                        format!("Input ({}) :: {}", identifier, property)
                                    } else {
                                        format!("{} :: {}", identifier, property)
                                    }
                                }
                                _ => format!("{:?}", v).replace("\"", "\\\""),
                            };
                            format!("{}: {}", k, val_str)
                        })
                        .collect();
                    label.push_str(&props.join("\\n"));
                }
                format!("label=\"{}\"", label)
            } else {
                format!("label=\"{}\"", id)
            }
        };

        Ok(format!(
            "{}",
            Dot::with_attr_getters(
                &graph,
                &[Config::EdgeNoLabel, Config::NodeNoLabel],
                &|_, _| String::new(),
                &get_node_attributes
            )
        ))
    }

    pub async fn run_pipeline_async<I: NodeOutput, T: FromSharedValue>(
        &self,
        workflow: &str,
        input: I,
    ) -> Result<T, crate::node::NodeError> {
        let runtime_inputs_vec = input.into_outputs();
        let runtime_inputs: HashMap<String, SharedValue> = runtime_inputs_vec
            .into_iter()
            .map(|(key, value)| (key.to_string(), value))
            .collect();

        let ParsedWorkflow {
            mut nodes,
            inputs,
            event_handlers,
            event_callback_nodes,
            sequential_edges,
        } = NodeParser::new(workflow)
            .map_err(|error| crate::node::NodeError::ParseError(format!("{}", error)))?
            .with_inputs(runtime_inputs.clone())
            .evaluate()
            .map_err(|error| crate::node::NodeError::ParseError(format!("{}", error)))?;

        // Create nodes for inputs
        let mut input_names = std::collections::HashSet::new();

        for (name, default_value) in &inputs {
            input_names.insert(name.clone());

            let node_name = format!("__input_{}", name);
            let mut instruct = NodeInstruct::new(Some(&node_name), "__input__");

            if let Some(val) = default_value {
                instruct.inputs.insert("default".to_string(), val.clone());
            }

            nodes.insert(node_name, instruct);
        }

        // Validate that all modules are either registered or are ignored (_)
        self.validate_modules(&nodes)?;

        let has_result = nodes.contains_key(PIPELINE_RESULT_ID);

        let (graph, _) = build_dependency_graph(&nodes, &event_handlers, &sequential_edges);
        let levels = compute_execution_levels(&graph);

        let mut outputs: HashMap<Identifier, HashMap<String, SharedValue>> = HashMap::new();

        // Pre-populate outputs from data-only nodes (ignored modules or unregistered for legacy support).
        // These act as constant/config holders whose literal inputs are
        // made available as outputs for expression references.
        // Note: modules named "_" are explicitly treated as data-only holders.
        // Skip the pipeline result node — it must be resolved after all execution.

        // Phase 1: Resolve inputs first
        for (id, instruct) in &nodes {
            if instruct.module == "__input__" {
                let payload = resolve_inputs(instruct, &outputs, &nodes, &input_names)?;
                let default_value = payload.get("default").cloned();

                // The original input name is extracted from the node name (removing __input_ prefix)
                let name = id
                    .strip_prefix("__input_")
                    .ok_or_else(|| crate::node::NodeError::Custom(format!("invalid input node identifier: {}", id)))?;
                let value = if let Some(value) = runtime_inputs.get(name) {
                    value.clone()
                } else if let Some(value) = default_value {
                    value
                } else {
                    return Err(crate::node::NodeError::MissingInput(name.to_string()));
                };

                let mut map = HashMap::new();
                map.insert("output".to_string(), value);
                outputs.insert(id.clone(), map);
            }
        }

        // Phase 2: Resolve data-only nodes
        for (id, instruct) in &nodes {
            if id == PIPELINE_RESULT_ID {
                continue;
            }

            if event_callback_nodes.contains(id) {
                continue;
            }

            if instruct.module == "_" || !self.registry.has(&instruct.module) {
                // If it's an input, we already handled it
                if instruct.module == "__input__" {
                    continue;
                }

                outputs.insert(id.clone(), resolve_inputs(instruct, &outputs, &nodes, &input_names)?);
            }
        }

        {
            let mut callback_execution_context = callback_execution::CallbackExecutionContext::new(
                &self.registry,
                &nodes,
                &input_names,
                &mut outputs,
                &event_handlers,
            );

            for level_indices in levels {
                for idx in level_indices {
                    let id = &graph[idx];
                    let instruct = &nodes[id];

                    if instruct.module == "__input__" || event_callback_nodes.contains(id) {
                        continue;
                    }

                    if instruct.module == "_" || !self.registry.has(&instruct.module) {
                        continue;
                    }

                    let payload = resolve_inputs(instruct, callback_execution_context.outputs, &nodes, &input_names)?;
                    callback_execution_context.execute_node(id, instruct, payload).await?;
                }
            }
        }

        // Resolve and return the pipeline result if defined
        if has_result {
            let result_instruct = &nodes[PIPELINE_RESULT_ID];
            let resolved = resolve_inputs(result_instruct, &outputs, &nodes, &input_names)?;
            if let Some(value) = resolved.into_values().next() {
                T::from_shared_value(&value)
            } else {
                Err(crate::node::NodeError::PipelineResultNotResolved)
            }
        } else {
            // If no result is defined, try to convert from "empty/void"
            // We pass a dummy shared value (e.g. unit) and see if T accepts it.
            // But SharedValue is Arc<dyn Any>. We can pass Arc::new(()).
            let unit: SharedValue = Arc::new(());

            T::from_shared_value(&unit).map_err(|_| crate::node::NodeError::NoPipelineResultDefined)
        }
    }
}

fn resolve_input_fallback(
    identifier: &str,
    outputs: &HashMap<Identifier, HashMap<String, SharedValue>>,
) -> Option<SharedValue> {
    let input_node_id = format!("__input_{}", identifier);

    if let Some(dependency_outputs) = outputs.get(&input_node_id) {
        dependency_outputs.get("output").cloned()
    } else {
        None
    }
}

fn evaluate_string_interpolation(
    expression: &Expression,
    outputs: &HashMap<Identifier, HashMap<String, SharedValue>>,
    nodes: &BTreeMap<Identifier, NodeInstruct>,
    input_names: &std::collections::HashSet<String>,
) -> Result<String, crate::node::NodeError> {
    match expression {
        Expression::Reference { identifier, property } => {
            shared_value_to_string(&resolve_reference(identifier, property, outputs, nodes, input_names)?)
        }
        _ => Ok(evaluate_expression(expression, outputs, nodes, input_names)?.to_string()),
    }
}

fn resolve_string_parts(
    parts: &[StringPart],
    outputs: &HashMap<Identifier, HashMap<String, SharedValue>>,
    nodes: &BTreeMap<Identifier, NodeInstruct>,
    input_names: &std::collections::HashSet<String>,
) -> Result<String, crate::node::NodeError> {
    let mut result = String::new();

    for part in parts {
        match part {
            StringPart::Literal(string) => result.push_str(string),
            StringPart::Interpolation(expression) => {
                result.push_str(&evaluate_string_interpolation(expression, outputs, nodes, input_names)?);
            }
        }
    }

    Ok(result)
}

fn is_input_direction(value: &Value) -> bool {
    match value {
        Value::String { direction, .. }
        | Value::Numeric { direction, .. }
        | Value::Boolean { direction, .. }
        | Value::Expression { direction, .. }
        | Value::Relation { direction, .. }
        | Value::Tuple { direction, .. }
        | Value::Range { direction, .. } => *direction == Direction::Input,
    }
}

/// Resolve a node's inputs from literal values and dependency outputs.
fn resolve_inputs(
    instruct: &NodeInstruct,
    outputs: &HashMap<Identifier, HashMap<String, SharedValue>>,
    nodes: &BTreeMap<Identifier, NodeInstruct>,
    input_names: &std::collections::HashSet<String>,
) -> Result<Payload, crate::node::NodeError> {
    let mut payload = HashMap::new();

    for (name, value) in &instruct.inputs {
        if is_input_direction(value) {
            payload.insert(name.clone(), resolve_single_value(value, outputs, nodes, input_names)?);
        }
    }

    Ok(payload)
}

fn resolve_single_value(
    value: &Value,
    outputs: &HashMap<Identifier, HashMap<String, SharedValue>>,
    nodes: &BTreeMap<Identifier, NodeInstruct>,
    input_names: &std::collections::HashSet<String>,
) -> Result<SharedValue, crate::node::NodeError> {
    match value {
        Value::String { parts, .. } => Ok(Arc::new(resolve_string_parts(parts, outputs, nodes, input_names)?)),
        Value::Numeric { value, .. } => {
            // Try parsing as integer (i128) then float (f64)
            if let Ok(i) = value.parse::<i128>() {
                Ok(Arc::new(i) as SharedValue)
            } else if let Ok(f) = value.parse::<f64>() {
                Ok(Arc::new(f) as SharedValue)
            } else {
                Err(crate::node::NodeError::Custom(format!(
                    "invalid numeric literal '{}'",
                    value
                )))
            }
        }
        Value::Expression { value, .. } => {
            let result = evaluate_expression(value, outputs, nodes, input_names)?;

            if result >= 0.0 && result <= u32::MAX as f64 {
                Ok(Arc::new(result as u32) as SharedValue)
            } else {
                Ok(Arc::new(result) as SharedValue)
            }
        }
        Value::Boolean { value, .. } => Ok(Arc::new(*value) as SharedValue),
        Value::Relation {
            identifier, property, ..
        } => resolve_reference(identifier, property, outputs, nodes, input_names),
        Value::Tuple { values, .. } => {
            let mut resolved = Vec::new();

            for value in values {
                resolved.push(resolve_single_value(value, outputs, nodes, input_names)?);
            }

            Ok(Arc::new(resolved) as SharedValue)
        }
        Value::Range {
            start, end, inclusive, ..
        } => {
            if *inclusive {
                Ok(Arc::new(*start..=*end) as SharedValue)
            } else {
                Ok(Arc::new(*start..*end) as SharedValue)
            }
        }
    }
}

/// Collect all node references from an expression tree.
pub(super) fn collect_expression_references(expression: &Expression) -> Vec<(&str, &str)> {
    match expression {
        Expression::Number(_) => vec![],
        Expression::String(_) => vec![],
        Expression::Reference { identifier, property } => vec![(identifier.as_str(), property.as_str())],
        Expression::BinaryOperation { left, right, .. } => {
            let mut references = collect_expression_references(left);
            references.extend(collect_expression_references(right));
            references
        }
    }
}

/// Evaluate an expression tree, resolving node references from dependency outputs
/// or falling back to literal values from parsed node instructions.
fn evaluate_expression(
    expression: &Expression,
    outputs: &HashMap<Identifier, HashMap<String, SharedValue>>,
    nodes: &BTreeMap<Identifier, NodeInstruct>,
    input_names: &std::collections::HashSet<String>,
) -> Result<f64, crate::node::NodeError> {
    match expression {
        Expression::Number(value) => value.parse::<f64>().map_err(|error| {
            crate::node::NodeError::Custom(format!("invalid numeric expression '{}': {}", value, error))
        }),
        Expression::String(_) => Err(crate::node::NodeError::NotANumericType),
        Expression::Reference { identifier, property } => {
            shared_value_to_f64(&resolve_reference(identifier, property, outputs, nodes, input_names)?)
        }
        Expression::BinaryOperation { operation, left, right } => {
            let left = evaluate_expression(left, outputs, nodes, input_names)?;
            let right = evaluate_expression(right, outputs, nodes, input_names)?;

            match operation {
                Operation::Add => Ok(left + right),
                Operation::Subtract => Ok(left - right),
                Operation::Multiply => Ok(left * right),
                Operation::Divide => Ok(left / right),
                Operation::Power => Ok(left.powf(right)),
            }
        }
    }
}

fn resolve_reference(
    identifier: &str,
    property: &str,
    outputs: &HashMap<Identifier, HashMap<String, SharedValue>>,
    nodes: &BTreeMap<Identifier, NodeInstruct>,
    input_names: &std::collections::HashSet<String>,
) -> Result<SharedValue, crate::node::NodeError> {
    // Try resolved outputs first (from executed nodes)
    if let Some(dependency_outputs) = outputs.get(identifier) {
        if let Some(value) = dependency_outputs.get(property) {
            return Ok(value.clone());
        }
    }

    // Try global input fallback if property is default "output" or "input" (depending on how it was parsed)
    if property == "output" && input_names.contains(identifier) {
        if let Some(value) = resolve_input_fallback(identifier, outputs) {
            return Ok(value);
        }
    }

    // Fall back to literal inputs from parsed node (data-only nodes)
    if let Some(instruct) = nodes.get(identifier) {
        if let Some(value) = instruct.inputs.get(property) {
            return match value {
                Value::Numeric { value, .. } => {
                    if let Ok(i) = value.parse::<i128>() {
                        Ok(Arc::new(i as f64)) // For expressions, we still need f64 currently
                    } else {
                        value
                            .parse::<f64>()
                            .map(|numeric| Arc::new(numeric) as SharedValue)
                            .map_err(|error| {
                                crate::node::NodeError::Custom(format!(
                                    "invalid numeric reference '{}::{}': {}",
                                    identifier, property, error
                                ))
                            })
                    }
                }
                Value::String { parts, .. } => Ok(Arc::new(resolve_string_parts(parts, outputs, nodes, input_names)?)),
                Value::Boolean { value, .. } => Ok(Arc::new(*value)),
                Value::Expression { value, .. } => {
                    Ok(Arc::new(evaluate_expression(value, outputs, nodes, input_names)?))
                }
                Value::Range {
                    start, end, inclusive, ..
                } => {
                    if *inclusive {
                        Ok(Arc::new(*start..=*end))
                    } else {
                        Ok(Arc::new(*start..*end))
                    }
                }
                _ => Err(crate::node::NodeError::ReferenceTypeNotSupported {
                    identifier: identifier.to_string(),
                    property: property.to_string(),
                }),
            };
        }
    }

    Err(crate::node::NodeError::ReferenceResolutionError {
        identifier: identifier.to_string(),
        property: property.to_string(),
    })
}

fn shared_value_to_string(value: &SharedValue) -> Result<String, crate::node::NodeError> {
    if let Some(value) = value.downcast_ref::<String>() {
        Ok(value.clone())
    } else if let Some(value) = value.downcast_ref::<&str>() {
        Ok(value.to_string())
    } else {
        Ok(shared_value_to_f64(value)?.to_string())
    }
}

fn shared_value_to_f64(value: &SharedValue) -> Result<f64, crate::node::NodeError> {
    if let Some(value) = value.downcast_ref::<f64>() {
        return Ok(*value);
    }

    if let Some(value) = value.downcast_ref::<f32>() {
        return Ok(*value as f64);
    }

    macro_rules! check_numeric {
        ($($t:ty),*) => {
            $(
                if let Some(value) = value.downcast_ref::<$t>() {
                    return Ok(*value as f64);
                }
            )*
        };
    }

    check_numeric!(u8, u16, u32, u64, u128, usize, i8, i16, i32, i64, i128, isize);

    Err(crate::node::NodeError::NotANumericType)
}
