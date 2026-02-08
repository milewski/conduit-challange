use crate::dsl::parser::{
    Direction, Expression, Identifier, NodeInstruct, NodeParser, Operation, PIPELINE_RESULT_ID, ParsedWorkflow,
    StringPart, Value,
};
use crate::node::FromSharedValue;
use crate::node::SharedValue;
use crate::registry::{NodeRegistry, Payload};
use crate::traits::NodeOutput;
use petgraph::Direction as GraphDirection;
use petgraph::graph::{DiGraph, NodeIndex};
use std::collections::{BTreeMap, HashMap};
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

    pub fn validate_modules(&self, nodes: &BTreeMap<Identifier, NodeInstruct>) -> Result<(), String> {
        for (_, instruct) in nodes {
            let module_name = &instruct.module;
            if module_name != "_" && module_name != "__input__" && !self.registry.has(module_name) {
                return Err(format!(
                    "Unknown module '{}' in node '{}'",
                    module_name, instruct.identifier
                ));
            }
        }
        Ok(())
    }

    pub async fn run_pipeline_async<I: NodeOutput, T: FromSharedValue>(
        &self,
        workflow: &str,
        input: I,
    ) -> Result<T, crate::node::NodeError> {
        let ParsedWorkflow {
            mut nodes,
            inputs: input_definitions,
        } = NodeParser::parse(workflow).unwrap();

        // Create nodes for inputs
        let mut input_names = std::collections::HashSet::new();

        for (name, default_value) in &input_definitions {
            input_names.insert(name.clone());

            let node_name = format!("__input_{}", name);
            let mut instruct = NodeInstruct::new(Some(&node_name), "__input__");

            if let Some(val) = default_value {
                instruct.inputs.insert("default".to_string(), val.clone());
            }

            nodes.insert(node_name, instruct);
        }

        // Validate that all modules are either registered or are ignored (_)
        if let Err(e) = self.validate_modules(&nodes) {
            panic!("{}", e);
        }

        let has_result = nodes.contains_key(PIPELINE_RESULT_ID);

        let (graph, _index_map) = build_dependency_graph(&nodes);
        let levels = compute_execution_levels(&graph);

        let mut outputs: HashMap<Identifier, HashMap<String, SharedValue>> = HashMap::new();

        // Prepare runtime inputs
        let runtime_inputs_vec = input.into_outputs();
        let runtime_inputs: HashMap<String, SharedValue> = runtime_inputs_vec
            .into_iter()
            .map(|(k, v)| (k.to_string(), v))
            .collect();

        // Pre-populate outputs from data-only nodes (ignored modules or unregistered for legacy support).
        // These act as constant/config holders whose literal inputs are
        // made available as outputs for expression references.
        // Note: modules named "_" are explicitly treated as data-only holders.
        // Skip the pipeline result node — it must be resolved after all execution.
        for (id, instruct) in &nodes {
            if id == PIPELINE_RESULT_ID {
                continue;
            }

            if instruct.module == "_" || !self.registry.has(&instruct.module) {
                outputs.insert(id.clone(), resolve_inputs(instruct, &outputs, &nodes, &input_names));
            }
        }

        for level_indices in levels {
            let mut handles = Vec::new();
            let mut handle_ids = Vec::new();

            for idx in level_indices {
                let id = &graph[idx];
                let instruct = &nodes[id];

                if instruct.module == "__input__" {
                    let payload = resolve_inputs(instruct, &outputs, &nodes, &input_names);
                    let default_value = payload.get("default").cloned();

                    // The original input name is extracted from the node name (removing __input_ prefix)
                    let name = id.strip_prefix("__input_").unwrap();
                    let value = if let Some(v) = runtime_inputs.get(name) {
                        v.clone()
                    } else if let Some(v) = default_value {
                        v
                    } else {
                        // If no default value and no runtime input, this is an error?
                        // For now we panic as per other error handling in this file
                        panic!("Missing input '{}'", name);
                    };

                    let mut map = HashMap::new();
                    map.insert("output".to_string(), value);
                    outputs.insert(id.clone(), map);
                    continue;
                }

                // Skip data-only nodes (marked with _ or others pre-resolved) — already resolved above
                if instruct.module == "_" || !self.registry.has(&instruct.module) {
                    continue;
                }

                let payload = resolve_inputs(instruct, &outputs, &nodes, &input_names);
                let module_name = instruct.module.clone();
                let registry = self.registry.clone();

                handle_ids.push(id.clone());
                handles.push(tokio::spawn(async move {
                    let instance = registry
                        .create(&module_name, Default::default())
                        .unwrap_or_else(|| panic!("Module '{}' not found in registry", module_name));

                    instance
                        .run_with_payload(payload)
                        .await
                        .unwrap_or_else(|error| panic!("Node execution failed: {}", error))
                }));
            }

            for (handle, id) in handles.into_iter().zip(handle_ids) {
                let node_outputs = handle.await.unwrap();
                let map: HashMap<String, SharedValue> = node_outputs
                    .into_iter()
                    .map(|(key, value)| (key.to_string(), value))
                    .collect();

                outputs.insert(id, map);
            }
        }

        // Resolve and return the pipeline result if defined
        if has_result {
            let result_instruct = &nodes[PIPELINE_RESULT_ID];
            let resolved = resolve_inputs(result_instruct, &outputs, &nodes, &input_names);
            if let Some(value) = resolved.into_values().next() {
                T::from_shared_value(&value)
            } else {
                Err(crate::node::NodeError::Custom(
                    "Pipeline result not resolved".to_string(),
                ))
            }
        } else {
            Err(crate::node::NodeError::Custom(
                "No pipeline result defined (no `<- value` statement)".to_string(),
            ))
        }
    }
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
        for (prop, value) in &instruct.inputs {
            match value {
                Value::Relation {
                    identifier: dep_id,
                    property,
                    direction,
                } => match direction {
                    Direction::Input => {
                        let dep_idx = index_map
                            .get(dep_id)
                            .or_else(|| index_map.get(&format!("__input_{}", dep_id)));

                        if let (Some(&from), Some(&to)) = (dep_idx, index_map.get(id)) {
                            graph.add_edge(from, to, property.clone());
                        }
                    }
                    Direction::Output => {
                        let dep_idx = index_map
                            .get(dep_id)
                            .or_else(|| index_map.get(&format!("__input_{}", dep_id)));

                        if let (Some(&from), Some(&to)) = (index_map.get(id), dep_idx) {
                            graph.add_edge(from, to, prop.clone());
                        }
                    }
                },
                Value::String { parts, .. } => {
                    for part in parts {
                        if let StringPart::Interpolation(expr) = part {
                            for (ref_id, _) in collect_expression_references(expr) {
                                let ref_idx = index_map
                                    .get(ref_id)
                                    .or_else(|| index_map.get(&format!("__input_{}", ref_id)));

                                if let (Some(&from), Some(&to)) = (ref_idx, index_map.get(id)) {
                                    graph.add_edge(from, to, prop.clone());
                                }
                            }
                        }
                    }
                }
                Value::Expression { value: expr, .. } => {
                    for (ref_id, _) in collect_expression_references(expr) {
                        let ref_idx = index_map
                            .get(ref_id)
                            .or_else(|| index_map.get(&format!("__input_{}", ref_id)));

                        if let (Some(&from), Some(&to)) = (ref_idx, index_map.get(id)) {
                            graph.add_edge(from, to, prop.clone());
                        }
                    }
                }
                _ => {}
            }
        }
    }

    (graph, index_map)
}

/// Compute execution levels via topological sort.
/// Nodes at the same level have no mutual dependencies and can run in parallel.
fn compute_execution_levels(graph: &DiGraph<Identifier, String>) -> Vec<Vec<NodeIndex>> {
    let topo = petgraph::algo::toposort(graph, None).expect("Cycle detected in dependency graph");

    let mut node_levels: HashMap<NodeIndex, usize> = HashMap::new();

    for &idx in &topo {
        let max_dep_level = graph
            .neighbors_directed(idx, GraphDirection::Incoming)
            .filter_map(|dep| node_levels.get(&dep).copied())
            .max();

        let level = match max_dep_level {
            Some(level) => level + 1,
            None => 0,
        };

        node_levels.insert(idx, level);
    }

    let max_level = node_levels.values().max().copied().unwrap_or(0);
    let mut levels = vec![Vec::new(); max_level + 1];

    for &idx in &topo {
        levels[node_levels[&idx]].push(idx);
    }

    levels
}

/// Resolve a node's inputs from literal values and dependency outputs.
fn resolve_inputs(
    instruct: &NodeInstruct,
    outputs: &HashMap<Identifier, HashMap<String, SharedValue>>,
    nodes: &BTreeMap<Identifier, NodeInstruct>,
    input_names: &std::collections::HashSet<String>,
) -> Payload {
    let mut payload = HashMap::new();

    for (name, value) in &instruct.inputs {
        match value {
            Value::String { parts, direction, .. } if *direction == Direction::Input => {
                let mut result = String::new();
                for part in parts {
                    match part {
                        StringPart::Literal(s) => result.push_str(s),
                        StringPart::Interpolation(expr) => match expr {
                            Expression::Reference { identifier, property } => {
                                let val = resolve_reference(identifier, property, outputs, nodes, input_names);
                                result.push_str(&shared_value_to_string(&val));
                            }
                            _ => {
                                let val = evaluate_expression(expr, outputs, nodes, input_names);
                                result.push_str(&val.to_string());
                            }
                        },
                    }
                }
                payload.insert(name.clone(), Arc::new(result) as SharedValue);
            }
            Value::Numeric { value, direction, .. } if *direction == Direction::Input => {
                payload.insert(name.clone(), Arc::new(value.parse::<u32>().unwrap()) as SharedValue);
            }
            Value::Expression { value: expr, direction } if *direction == Direction::Input => {
                let result = evaluate_expression(expr, outputs, nodes, input_names);
                if result >= 0.0 && result <= u32::MAX as f64 {
                    payload.insert(name.clone(), Arc::new(result as u32) as SharedValue);
                } else {
                    payload.insert(name.clone(), Arc::new(result) as SharedValue);
                }
            }
            Value::Boolean { value, direction, .. } if *direction == Direction::Input => {
                payload.insert(name.clone(), Arc::new(*value) as SharedValue);
            }
            Value::Relation {
                identifier,
                property,
                direction,
            } if *direction == Direction::Input => {
                if let Some(dep_outputs) = outputs.get(identifier) {
                    if let Some(val) = dep_outputs.get(property) {
                        payload.insert(name.clone(), val.clone());
                    } else if property == "output" && input_names.contains(identifier) {
                        // Fallback to global input if property is output (or implied)
                        // And identifier is an input name
                        let input_node_id = format!("__input_{}", identifier);
                        if let Some(dep_outputs) = outputs.get(&input_node_id) {
                            if let Some(val) = dep_outputs.get("output") {
                                payload.insert(name.clone(), val.clone());
                            }
                        }
                    }
                } else if input_names.contains(identifier) {
                    // Not found in outputs (maybe didn't run yet? or not a node?)
                    // If it is an input, check the input node output
                    let input_node_id = format!("__input_{}", identifier);
                    if let Some(dep_outputs) = outputs.get(&input_node_id) {
                        if let Some(val) = dep_outputs.get("output") {
                            payload.insert(name.clone(), val.clone());
                        }
                    }
                }
            }
            _ => {}
        }
    }

    payload
}

/// Collect all node references from an expression tree.
fn collect_expression_references(expression: &Expression) -> Vec<(&str, &str)> {
    match expression {
        Expression::Number(_) => vec![],
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
) -> f64 {
    match expression {
        Expression::Number(value) => value.parse::<f64>().unwrap(),
        Expression::Reference { identifier, property } => {
            let value = resolve_reference(identifier, property, outputs, nodes, input_names);
            shared_value_to_f64(&value)
        }
        Expression::BinaryOperation { operation, left, right } => {
            let left = evaluate_expression(left, outputs, nodes, input_names);
            let right = evaluate_expression(right, outputs, nodes, input_names);

            match operation {
                Operation::Add => left + right,
                Operation::Subtract => left - right,
                Operation::Multiply => left * right,
                Operation::Divide => left / right,
                Operation::Power => left.powf(right),
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
) -> SharedValue {
    // Try resolved outputs first (from executed nodes)
    if let Some(dep_outputs) = outputs.get(identifier) {
        if let Some(value) = dep_outputs.get(property) {
            return value.clone();
        }
    }

    // Try global input fallback if property is default "output" or "input" (depending on how it was parsed)
    if property == "output" && input_names.contains(identifier) {
        let input_node_id = format!("__input_{}", identifier);
        if let Some(dep_outputs) = outputs.get(&input_node_id) {
            if let Some(value) = dep_outputs.get("output") {
                return value.clone();
            }
        }
    }

    // Fall back to literal inputs from parsed node (data-only nodes)
    if let Some(instruct) = nodes.get(identifier) {
        if let Some(value) = instruct.inputs.get(property) {
            return match value {
                Value::Numeric { value, .. } => Arc::new(value.parse::<f64>().unwrap()),
                Value::String { parts, .. } => {
                    // Reconstruct string value (no interpolation support in recursion yet? or assume literal?)
                    // If we are referencing a string from an expression, it likely shouldn't happen unless we support string ops.
                    // But if we are just resolving it, we can return it.
                    // CAUTION: If the referenced string HAS interpolation, we should evaluate it.
                    // But evaluate_expression returns f64. This path is called from resolve_reference.
                    // We can recursively call resolve_inputs logic? But resolve_inputs is for a whole node.

                    // Simplify: Assume referenced string literals in data-only nodes are just literals for now
                    // OR implement simple evaluation.
                    let mut result = String::new();
                    for part in parts {
                        match part {
                            StringPart::Literal(s) => result.push_str(s),
                            StringPart::Interpolation(expr) => {
                                // Recurse
                                // But wait, if we are in resolve_reference, we might cause infinite loop if cycle?
                                // DAG check handles cycles.
                                match expr {
                                    Expression::Reference { identifier, property } => {
                                        let val = resolve_reference(identifier, property, outputs, nodes, input_names);
                                        result.push_str(&shared_value_to_string(&val));
                                    }
                                    _ => {
                                        let val = evaluate_expression(expr, outputs, nodes, input_names);
                                        result.push_str(&val.to_string());
                                    }
                                }
                            }
                        }
                    }
                    Arc::new(result)
                }
                Value::Boolean { value, .. } => Arc::new(*value),
                Value::Expression { value: inner, .. } => {
                    Arc::new(evaluate_expression(inner, outputs, nodes, input_names))
                }
                _ => panic!("Reference '{}::{}' type not supported", identifier, property),
            };
        }
    }
    panic!("Cannot resolve '{}::{}'", identifier, property);
}

fn shared_value_to_string(value: &SharedValue) -> String {
    if let Some(v) = value.downcast_ref::<String>() {
        v.clone()
    } else if let Some(v) = value.downcast_ref::<&str>() {
        v.to_string()
    } else {
        shared_value_to_f64(value).to_string()
    }
}

fn shared_value_to_f64(value: &SharedValue) -> f64 {
    if let Some(v) = value.downcast_ref::<f64>() {
        *v
    } else if let Some(v) = value.downcast_ref::<u32>() {
        *v as f64
    } else if let Some(v) = value.downcast_ref::<i32>() {
        *v as f64
    } else if let Some(v) = value.downcast_ref::<i64>() {
        *v as f64
    } else if let Some(v) = value.downcast_ref::<f32>() {
        *v as f64
    } else {
        panic!("Expression reference value is not a numeric type")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        let payload = resolve_inputs(instruct, &outputs, &parsed.nodes, &input_names);

        assert_eq!(payload.len(), 3);
        assert_eq!(*payload["s"].downcast_ref::<String>().unwrap(), "hello");
        assert_eq!(*payload["n"].downcast_ref::<u32>().unwrap(), 42);
        assert_eq!(*payload["b"].downcast_ref::<bool>().unwrap(), true);
    }

    #[test]
    fn test_resolve_expression_literal_only() {
        let parsed = NodeParser::parse(r#"a module_a { x <- (10 + 20) }"#).unwrap();

        let instruct = &parsed.nodes[&String::from("a")];
        let outputs = HashMap::new();
        let input_names = std::collections::HashSet::new();
        let payload = resolve_inputs(instruct, &outputs, &parsed.nodes, &input_names);

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
        let payload = resolve_inputs(instruct, &outputs, &parsed.nodes, &input_names);

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
        let payload = resolve_inputs(instruct, &outputs, &parsed.nodes, &input_names);

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
        let payload = resolve_inputs(instruct, &outputs, &parsed.nodes, &input_names);

        // 10 + 3 * 2 = 16 (respects operator precedence)
        assert_eq!(*payload["z"].downcast_ref::<u32>().unwrap(), 16);
    }

    #[test]
    #[should_panic(expected = "Unknown module 'invalid'")]
    fn test_invalid_module_panics() {
        let engine = Engine::new();
        let parsed = NodeParser::parse(r#"a invalid { x <- 1 }"#).unwrap();

        // This should return Err, but we panic on error
        if let Err(e) = engine.validate_modules(&parsed.nodes) {
            panic!("{}", e);
        }
    }

    #[test]
    fn test_ignored_module_underscore() {
        let parsed = NodeParser::parse(r#"config _ { multiplier <- 5 }"#).unwrap();

        let instruct = &parsed.nodes[&String::from("config")];
        let outputs = HashMap::new();
        let input_names = std::collections::HashSet::new();
        let payload = resolve_inputs(instruct, &outputs, &parsed.nodes, &input_names);

        // Should work without error, just returns the literal value
        assert_eq!(*payload["multiplier"].downcast_ref::<u32>().unwrap(), 5);
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
        assert!(result.is_err());
    }

    #[test]
    fn test_pipeline_result_type_mismatch() {
        let mut engine = Engine::new();
        let pipeline = r#"<- "hello""#;
        let result: Result<u32, _> = engine.run_pipeline_blocking(pipeline, ());
        assert!(result.is_err());
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

        let payload = resolve_inputs(instruct, &outputs, &parsed.nodes, &input_names);

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
        let payload = resolve_inputs(instruct, &outputs, &parsed.nodes, &input_names);

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
        let payload = resolve_inputs(instruct, &outputs, &parsed.nodes, &input_names);

        assert_eq!(*payload["msg"].downcast_ref::<String>().unwrap(), "hello world");
    }
}
