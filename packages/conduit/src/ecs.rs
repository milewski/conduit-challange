use crate::dsl::parser::{
    CallbackAssignment, Direction, EventCallback, Expression, Identifier, NodeInstruct, NodeParser, Operation,
    PIPELINE_RESULT_ID, ParsedWorkflow, StringPart, Value,
};
use crate::node::FromSharedValue;
use crate::node::SharedValue;
use crate::registry::{NodeRegistry, Payload};
use crate::traits::NodeOutput;
use petgraph::Direction as GraphDirection;
use petgraph::dot::{Config, Dot};
use petgraph::graph::{DiGraph, NodeIndex};
use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};
use std::sync::Arc;

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
            event_handlers: _,
            event_callback_nodes: _,
        } = NodeParser::parse(workflow).map_err(|error| crate::node::NodeError::ParseError(format!("{:?}", error)))?;

        let (graph, _) = build_dependency_graph(&nodes);

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
        } = NodeParser::new(workflow)
            .map_err(|error| crate::node::NodeError::ParseError(format!("{:?}", error)))?
            .with_inputs(runtime_inputs.clone())
            .evaluate()
            .map_err(|error| crate::node::NodeError::ParseError(format!("{:?}", error)))?;

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

        let (graph, _) = build_dependency_graph(&nodes);
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
                let name = id.strip_prefix("__input_").unwrap();
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

            if instruct.module == "_" || !self.registry.has(&instruct.module) {
                // If it's an input, we already handled it
                if instruct.module == "__input__" {
                    continue;
                }

                outputs.insert(id.clone(), resolve_inputs(instruct, &outputs, &nodes, &input_names)?);
            }
        }

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

                let payload = resolve_inputs(instruct, &outputs, &nodes, &input_names)?;
                let instance = self
                    .registry
                    .create(&instruct.module, Default::default())
                    .ok_or_else(|| crate::node::NodeError::ModuleNotFound(instruct.module.clone()))?;

                let execution_result = instance.run_with_payload(payload).await?;
                let map: HashMap<String, SharedValue> = execution_result
                    .outputs
                    .into_iter()
                    .map(|(key, value)| (key.to_string(), value))
                    .collect();

                outputs.insert(id.clone(), map);

                run_event_callbacks(
                    &self.registry,
                    &nodes,
                    &input_names,
                    &mut outputs,
                    &event_handlers,
                    event_handlers.get(id),
                    execution_result.events,
                )
                .await?;
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

async fn run_event_callbacks(
    registry: &Arc<NodeRegistry>,
    nodes: &BTreeMap<Identifier, NodeInstruct>,
    input_names: &std::collections::HashSet<String>,
    outputs: &mut HashMap<Identifier, HashMap<String, SharedValue>>,
    all_event_handlers: &BTreeMap<Identifier, BTreeMap<String, Vec<EventCallback>>>,
    event_handlers: Option<&BTreeMap<String, Vec<EventCallback>>>,
    emitted_events: Vec<crate::traits::EventData>,
) -> Result<(), crate::node::NodeError> {
    let mut queued_callbacks: VecDeque<(EventCallback, Option<SharedValue>)> = VecDeque::new();

    for emitted_event in emitted_events {
        if let Some(callbacks) = event_handlers.and_then(|handlers| handlers.get(&emitted_event.name)) {
            for callback in callbacks {
                queued_callbacks.push_back((callback.clone(), emitted_event.value.clone()));
            }
        }
    }

    while let Some((event_callback, event_payload)) = queued_callbacks.pop_front() {
        match event_callback {
            EventCallback::Value(callback_value) => {
                if let Value::Relation {
                    identifier, property, ..
                } = callback_value
                {
                    if let Some(callback_node) = nodes.get(&identifier) {
                        if callback_node.module != "_" && registry.has(&callback_node.module) {
                            let mut callback_payload = resolve_inputs(callback_node, outputs, nodes, input_names)?;

                            if let Some(value) = event_payload.clone() {
                                callback_payload.insert(property, value);
                            }

                            let callback_instance = registry
                                .create(&callback_node.module, Default::default())
                                .ok_or_else(|| crate::node::NodeError::ModuleNotFound(callback_node.module.clone()))?;
                            let callback_result = callback_instance.run_with_payload(callback_payload).await?;
                            let callback_outputs: HashMap<String, SharedValue> = callback_result
                                .outputs
                                .into_iter()
                                .map(|(key, value)| (key.to_string(), value))
                                .collect();
                            outputs.insert(identifier.clone(), callback_outputs);

                            for emitted_event in callback_result.events {
                                if let Some(callbacks) = all_event_handlers
                                    .get(&identifier)
                                    .and_then(|handlers| handlers.get(&emitted_event.name))
                                {
                                    for callback in callbacks {
                                        queued_callbacks.push_back((callback.clone(), emitted_event.value.clone()));
                                    }
                                }
                            }
                        } else if let Some(value) = event_payload.clone() {
                            outputs.entry(identifier).or_default().insert(property, value);
                        }
                    } else if let Some(value) = event_payload.clone() {
                        outputs.entry(identifier).or_default().insert(property, value);
                    }
                } else {
                    let _ = resolve_single_value(&callback_value, outputs, nodes, input_names)?;
                }
            }
            EventCallback::Assignment(CallbackAssignment {
                identifier,
                property,
                value,
            }) => {
                let resolved_value = resolve_single_value(&value, outputs, nodes, input_names)?;
                outputs.entry(identifier).or_default().insert(property, resolved_value);
            }
            EventCallback::Block(callbacks) => {
                for callback in callbacks {
                    queued_callbacks.push_back((callback, event_payload.clone()));
                }
            }
        }
    }

    Ok(())
}

/// Build a directed dependency graph from parsed nodes.
/// Edges point from dependency → dependent (data flow direction).
fn build_dependency_graph(
    nodes: &BTreeMap<Identifier, NodeInstruct>,
) -> (DiGraph<Identifier, String>, HashMap<Identifier, NodeIndex>) {
    let mut graph = DiGraph::new();
    let mut index_map = HashMap::new();

    for (id, _) in nodes {
        let idx = graph.add_node(id.clone());
        index_map.insert(id.clone(), idx);
    }

    for (id, instruct) in nodes {
        for (property, value) in &instruct.inputs {
            match value {
                Value::Relation {
                    identifier,
                    property,
                    direction,
                } => match direction {
                    Direction::Input => {
                        add_dependency_edge(&mut graph, &index_map, identifier, id, property);
                    }
                    Direction::Output => {
                        add_dependency_edge(&mut graph, &index_map, id, identifier, property);
                    }
                },
                Value::String { parts, .. } => {
                    for part in parts {
                        if let StringPart::Interpolation(expression) = part {
                            for (reference_id, _) in collect_expression_references(expression) {
                                add_dependency_edge(&mut graph, &index_map, reference_id, id, property);
                            }
                        }
                    }
                }
                Value::Expression { value, .. } => {
                    for (reference_id, _) in collect_expression_references(value) {
                        add_dependency_edge(&mut graph, &index_map, reference_id, id, property);
                    }
                }
                Value::Tuple { values, .. } => {
                    for value in values {
                        collect_tuple_references(value, id, property, &index_map, &mut graph);
                    }
                }
                _ => {}
            }
        }
    }

    (graph, index_map)
}

fn add_dependency_edge(
    graph: &mut DiGraph<Identifier, String>,
    index_map: &HashMap<Identifier, NodeIndex>,
    from_id: &str,
    to_id: &str,
    property: &str,
) {
    let dependency_index = index_map
        .get(from_id)
        .or_else(|| index_map.get(&format!("__input_{}", from_id)));

    if let (Some(&from), Some(&to)) = (dependency_index, index_map.get(to_id)) {
        graph.add_edge(from, to, property.to_string());
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
        | Value::Tuple { direction, .. } => *direction == Direction::Input,
    }
}

fn collect_tuple_references(
    value: &Value,
    id: &str,
    property: &str,
    index_map: &HashMap<Identifier, NodeIndex>,
    graph: &mut DiGraph<Identifier, String>,
) {
    match value {
        Value::Relation {
            identifier, direction, ..
        } => match direction {
            Direction::Input => {
                add_dependency_edge(graph, index_map, identifier, id, property);
            }
            Direction::Output => {
                add_dependency_edge(graph, index_map, id, identifier, property);
            }
        },
        Value::Expression { value, .. } => {
            for (reference_id, _) in collect_expression_references(value) {
                add_dependency_edge(graph, index_map, reference_id, id, property);
            }
        }
        Value::String { parts, .. } => {
            for part in parts {
                if let StringPart::Interpolation(expression) = part {
                    for (reference_id, _) in collect_expression_references(expression) {
                        add_dependency_edge(graph, index_map, reference_id, id, property);
                    }
                }
            }
        }
        Value::Tuple { values, .. } => {
            for value in values {
                collect_tuple_references(value, id, property, index_map, graph);
            }
        }
        _ => {}
    }
}

/// Compute execution levels via topological sort.
/// Nodes at the same level have no mutual dependencies and can run in parallel.
fn compute_execution_levels(graph: &DiGraph<Identifier, String>) -> Vec<Vec<NodeIndex>> {
    let topo = petgraph::algo::toposort(graph, None).expect("Cycle detected in dependency graph");

    let mut node_levels: HashMap<NodeIndex, usize> = HashMap::new();

    for &index in &topo {
        let max_dep_level = graph
            .neighbors_directed(index, GraphDirection::Incoming)
            .filter_map(|dep| node_levels.get(&dep).copied())
            .max();

        let level = match max_dep_level {
            Some(level) => level + 1,
            None => 0,
        };

        node_levels.insert(index, level);
    }

    let max_level = node_levels.values().max().copied().unwrap_or(0);
    let mut levels = vec![Vec::new(); max_level + 1];

    for &index in &topo {
        levels[node_levels[&index]].push(index);
    }

    levels
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
                // Fallback or error?
                // Parser grammar ensures it's a number, so it should parse as f64 at least.
                Ok(Arc::new(value.parse::<f64>().expect("valid number")) as SharedValue)
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
    }
}

/// Collect all node references from an expression tree.
fn collect_expression_references(expression: &Expression) -> Vec<(&str, &str)> {
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
        Expression::Number(value) => Ok(value.parse::<f64>().unwrap()),
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
                        Ok(Arc::new(value.parse::<f64>().unwrap()))
                    }
                }
                Value::String { parts, .. } => Ok(Arc::new(resolve_string_parts(parts, outputs, nodes, input_names)?)),
                Value::Boolean { value, .. } => Ok(Arc::new(*value)),
                Value::Expression { value, .. } => {
                    Ok(Arc::new(evaluate_expression(value, outputs, nodes, input_names)?))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_dot_graph() {
        let engine = Engine::new();
        let dot = engine
            .generate_dot_graph(
                r#"
            a module_a { x <- 1 }
            b module_b { y <- a::x }
        "#,
            )
            .unwrap();

        assert!(dot.contains("digraph {"));
        assert!(dot.contains("label=\"module_a (a)"));
        assert!(dot.contains("label=\"module_b (b)"));
    }

    #[test]
    fn test_build_graph_no_dependencies() {
        let parsed = NodeParser::parse(
            r#"
                a module_a { x <- 1 }
                b module_b { y <- 2 }
                c module_c { z <- 3 }
            "#,
        )
        .unwrap();

        let (graph, _) = build_dependency_graph(&parsed.nodes);
        let levels = compute_execution_levels(&graph);

        assert_eq!(levels.len(), 1, "all independent nodes should be at level 0");
        assert_eq!(levels[0].len(), 3);
    }

    #[test]
    fn test_build_graph_linear_chain() {
        let parsed = NodeParser::parse(
            r#"
                a module_a { x <- 1 }
                b module_b { y <- a::x }
                c module_c { z <- b::y }
            "#,
        )
        .unwrap();

        let (graph, index_map) = build_dependency_graph(&parsed.nodes);
        let levels = compute_execution_levels(&graph);

        assert_eq!(levels.len(), 3, "linear chain should have 3 levels");
        assert_eq!(levels[0].len(), 1);
        assert_eq!(levels[1].len(), 1);
        assert_eq!(levels[2].len(), 1);

        let a_level = levels
            .iter()
            .position(|l| l.contains(&index_map[&String::from("a")]))
            .unwrap();
        let b_level = levels
            .iter()
            .position(|l| l.contains(&index_map[&String::from("b")]))
            .unwrap();
        let c_level = levels
            .iter()
            .position(|l| l.contains(&index_map[&String::from("c")]))
            .unwrap();
        assert!(a_level < b_level);
        assert!(b_level < c_level);
    }

    #[test]
    fn test_build_graph_diamond() {
        let parsed = NodeParser::parse(
            r#"
                source module_s { x <- 1 }
                left module_l { y <- source::x }
                right module_r { z <- source::x }
                sink module_k { w <- left::y }
            "#,
        )
        .unwrap();

        let (graph, index_map) = build_dependency_graph(&parsed.nodes);
        let levels = compute_execution_levels(&graph);

        let source_level = levels
            .iter()
            .position(|l| l.contains(&index_map[&String::from("source")]))
            .unwrap();
        let left_level = levels
            .iter()
            .position(|l| l.contains(&index_map[&String::from("left")]))
            .unwrap();
        let right_level = levels
            .iter()
            .position(|l| l.contains(&index_map[&String::from("right")]))
            .unwrap();
        let sink_level = levels
            .iter()
            .position(|l| l.contains(&index_map[&String::from("sink")]))
            .unwrap();

        assert_eq!(source_level, 0);
        assert_eq!(left_level, right_level, "left and right should be at the same level");
        assert!(sink_level > left_level);
    }

    #[test]
    fn test_resolve_literal_inputs() {
        let parsed = NodeParser::parse(
            r#"
                a module_a {
                    s <- "hello"
                    n <- 42
                    b <- true
                }
            "#,
        )
        .unwrap();

        let instruct = &parsed.nodes[&String::from("a")];
        let outputs = HashMap::new();
        let input_names = std::collections::HashSet::new();
        let payload = resolve_inputs(instruct, &outputs, &parsed.nodes, &input_names).unwrap();

        assert_eq!(payload.len(), 3);
        assert_eq!(*payload["s"].downcast_ref::<String>().unwrap(), "hello");
        assert_eq!(*payload["n"].downcast_ref::<i128>().unwrap(), 42);
        assert_eq!(*payload["b"].downcast_ref::<bool>().unwrap(), true);
    }

    #[test]
    fn test_resolve_expression_literal_only() {
        let parsed = NodeParser::parse(r#"a module_a { x <- (10 + 20) }"#).unwrap();

        let instruct = &parsed.nodes[&String::from("a")];
        let outputs = HashMap::new();
        let input_names = std::collections::HashSet::new();
        let payload = resolve_inputs(instruct, &outputs, &parsed.nodes, &input_names).unwrap();

        assert_eq!(*payload["x"].downcast_ref::<u32>().unwrap(), 30);
    }

    #[test]
    fn test_resolve_expression_with_ref() {
        let parsed = NodeParser::parse(
            r#"
                config constants { multiplier <- 4 }
                img resizer { width <- (32 * config::multiplier) }
            "#,
        )
        .unwrap();

        // Simulate config node having already produced its output
        let mut outputs: HashMap<Identifier, HashMap<String, SharedValue>> = HashMap::new();
        let mut config_out = HashMap::new();
        config_out.insert("multiplier".to_string(), Arc::new(4u32) as SharedValue);
        outputs.insert("config".to_string(), config_out);

        let instruct = &parsed.nodes[&String::from("img")];
        let input_names = std::collections::HashSet::new();
        let payload = resolve_inputs(instruct, &outputs, &parsed.nodes, &input_names).unwrap();

        assert_eq!(*payload["width"].downcast_ref::<u32>().unwrap(), 128);
    }

    #[test]
    fn test_expression_creates_dependency_edge() {
        let parsed = NodeParser::parse(
            r#"
                config constants { multiplier <- 4 }
                img resizer { width <- (32 * config::multiplier) }
            "#,
        )
        .unwrap();

        let (graph, index_map) = build_dependency_graph(&parsed.nodes);
        let levels = compute_execution_levels(&graph);

        let config_level = levels.iter().position(|l| l.contains(&index_map["config"])).unwrap();
        let img_level = levels.iter().position(|l| l.contains(&index_map["img"])).unwrap();

        assert!(config_level < img_level, "config must execute before img");
    }

    #[test]
    fn test_resolve_expression_with_division() {
        let parsed = NodeParser::parse(
            r#"
                config constants { size <- 545 }
                img resizer { height <- (config::size / 2) }
            "#,
        )
        .unwrap();

        // Simulate config node having already produced its output
        let mut outputs: HashMap<Identifier, HashMap<String, SharedValue>> = HashMap::new();
        let mut config_out = HashMap::new();
        config_out.insert("size".to_string(), Arc::new(545u32) as SharedValue);
        outputs.insert("config".to_string(), config_out);

        let instruct = &parsed.nodes[&String::from("img")];
        let input_names = std::collections::HashSet::new();
        let payload = resolve_inputs(instruct, &outputs, &parsed.nodes, &input_names).unwrap();

        // 545 / 2 = 272.5, which should be truncated to 272
        assert_eq!(*payload["height"].downcast_ref::<u32>().unwrap(), 272);
    }

    #[test]
    fn test_resolve_expression_multi_ref() {
        let parsed = NodeParser::parse(
            r#"
                a module_a { x <- 10 }
                b module_b { y <- 3 }
                c module_c { z <- (a::x + b::y * 2) }
            "#,
        )
        .unwrap();

        let mut outputs: HashMap<Identifier, HashMap<String, SharedValue>> = HashMap::new();
        let mut a_out = HashMap::new();
        a_out.insert("x".to_string(), Arc::new(10u32) as SharedValue);
        outputs.insert("a".to_string(), a_out);
        let mut b_out = HashMap::new();
        b_out.insert("y".to_string(), Arc::new(3u32) as SharedValue);
        outputs.insert("b".to_string(), b_out);

        let instruct = &parsed.nodes[&String::from("c")];
        let input_names = std::collections::HashSet::new();
        let payload = resolve_inputs(instruct, &outputs, &parsed.nodes, &input_names).unwrap();

        // 10 + 3 * 2 = 16 (respects operator precedence)
        assert_eq!(*payload["z"].downcast_ref::<u32>().unwrap(), 16);
    }

    #[test]
    fn test_invalid_module_error() {
        let engine = Engine::new();
        let parsed = NodeParser::parse(r#"a invalid { x <- 1 }"#).unwrap();

        let result = engine.validate_modules(&parsed.nodes);
        assert!(matches!(result, Err(crate::node::NodeError::ModuleValidationError(_))));
    }

    #[test]
    fn test_ignored_module_underscore() {
        let parsed = NodeParser::parse(r#"config _ { multiplier <- 5 }"#).unwrap();

        let instruct = &parsed.nodes[&String::from("config")];
        let outputs = HashMap::new();
        let input_names = std::collections::HashSet::new();
        let payload = resolve_inputs(instruct, &outputs, &parsed.nodes, &input_names).unwrap();

        // Should work without error, just returns the literal value
        assert_eq!(*payload["multiplier"].downcast_ref::<i128>().unwrap(), 5);
    }

    #[test]
    fn test_pipeline_result_string() {
        let mut engine = Engine::new();
        let pipeline = r#"<- "hello world""#;
        let result: String = engine.run_pipeline_blocking(pipeline, ()).unwrap();
        assert_eq!(result, "hello world");
    }

    #[test]
    fn test_pipeline_result_number() {
        let mut engine = Engine::new();
        let pipeline = r#"<- 42"#;
        let result: u32 = engine.run_pipeline_blocking(pipeline, ()).unwrap();
        assert_eq!(result, 42);
    }

    #[test]
    fn test_pipeline_result_boolean() {
        let mut engine = Engine::new();
        let pipeline = r#"<- true"#;
        let result: bool = engine.run_pipeline_blocking(pipeline, ()).unwrap();
        assert_eq!(result, true);
    }

    #[test]
    fn test_pipeline_result_node_reference() {
        let mut engine = Engine::new();
        let pipeline = r#"
            config _ { output <- "from config" }
            <- config
        "#;
        let result: String = engine.run_pipeline_blocking(pipeline, ()).unwrap();
        assert_eq!(result, "from config");
    }

    #[test]
    fn test_pipeline_no_result() {
        let mut engine = Engine::new();
        let pipeline = r#"config _ { value <- 42 }"#;
        let result: Result<String, _> = engine.run_pipeline_blocking(pipeline, ());
        assert!(matches!(result, Err(crate::node::NodeError::NoPipelineResultDefined)));
    }

    #[test]
    fn test_pipeline_result_type_mismatch() {
        let mut engine = Engine::new();
        let pipeline = r#"<- "hello""#;
        let result: Result<u32, _> = engine.run_pipeline_blocking(pipeline, ());
        assert!(matches!(result, Err(crate::node::NodeError::TypeMismatch { .. })));
    }

    #[test]
    fn test_resolve_external_inputs() {
        // Define a node that uses an external input
        let parsed = NodeParser::parse(
            r#"
                -> external_val <- 10
                node module {
                    val <- external_val
                }
            "#,
        )
        .unwrap();

        let instruct = &parsed.nodes[&String::from("node")];

        // Simulate outputs containing the injected input
        let mut outputs: HashMap<Identifier, HashMap<String, SharedValue>> = HashMap::new();
        let mut input_out = HashMap::new();
        input_out.insert("output".to_string(), Arc::new(99u32) as SharedValue); // 99 overrides default 10
        outputs.insert("__input_external_val".to_string(), input_out);

        let mut input_names = std::collections::HashSet::new();
        input_names.insert("external_val".to_string());

        let payload = resolve_inputs(instruct, &outputs, &parsed.nodes, &input_names).unwrap();

        assert_eq!(*payload["val"].downcast_ref::<u32>().unwrap(), 99);
    }

    #[test]
    fn test_string_interpolation_with_expression() {
        let parsed = NodeParser::parse(
            r#"
                config _ { width <- 100 }
                node module { filename <- "cover.{ (config::width + 1) }.png" }
            "#,
        )
        .unwrap();

        let instruct = &parsed.nodes[&String::from("node")];

        let mut outputs: HashMap<Identifier, HashMap<String, SharedValue>> = HashMap::new();
        let mut config_out = HashMap::new();
        config_out.insert("width".to_string(), Arc::new(100u32) as SharedValue);
        outputs.insert("config".to_string(), config_out);

        let input_names = std::collections::HashSet::new();
        let payload = resolve_inputs(instruct, &outputs, &parsed.nodes, &input_names).unwrap();

        assert_eq!(*payload["filename"].downcast_ref::<String>().unwrap(), "cover.101.png");
    }

    #[test]
    fn test_string_interpolation_with_string_value() {
        let parsed = NodeParser::parse(
            r#"
                config _ { name <- "world" }
                node module { msg <- "hello { config::name }" }
            "#,
        )
        .unwrap();

        let instruct = &parsed.nodes[&String::from("node")];

        let mut outputs: HashMap<Identifier, HashMap<String, SharedValue>> = HashMap::new();
        let mut config_out = HashMap::new();
        config_out.insert("name".to_string(), Arc::new("world".to_string()) as SharedValue);
        outputs.insert("config".to_string(), config_out);

        let input_names = std::collections::HashSet::new();
        let payload = resolve_inputs(instruct, &outputs, &parsed.nodes, &input_names).unwrap();

        assert_eq!(*payload["msg"].downcast_ref::<String>().unwrap(), "hello world");
    }
}
