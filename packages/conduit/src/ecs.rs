use crate::dsl::parser::{Direction, Expr, ExprOp, Identifier, NodeInstruct, NodeParser, Value};
use crate::node::SharedValue;
use crate::registry::{NodeRegistry, Payload};
use petgraph::graph::{DiGraph, NodeIndex};
use petgraph::Direction as GraphDirection;
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

    pub fn run_pipeline(&mut self, workflow: &str) {
        self.runtime.block_on(self.run_pipeline_async(workflow));
    }

    async fn run_pipeline_async(&self, workflow: &str) {
        let nodes = NodeParser::parse(workflow).unwrap();

        let (graph, _index_map) = build_dependency_graph(&nodes);
        let levels = compute_execution_levels(&graph);

        let mut outputs: HashMap<Identifier, HashMap<String, SharedValue>> = HashMap::new();

        // Pre-populate outputs from data-only nodes (unregistered modules).
        // These act as constant/config holders whose literal inputs are
        // made available as outputs for expression references.
        for (id, instruct) in &nodes {
            if !self.registry.has(&instruct.module) {
                outputs.insert(id.clone(), resolve_inputs(instruct, &outputs, &nodes));
            }
        }

        for level_indices in levels {
            let mut handles = Vec::new();
            let mut handle_ids = Vec::new();

            for idx in level_indices {
                let id = &graph[idx];
                let instruct = &nodes[id];

                // Skip data-only nodes — already resolved above
                if !self.registry.has(&instruct.module) {
                    continue;
                }

                let payload = resolve_inputs(instruct, &outputs, &nodes);
                let module_name = instruct.module.clone();
                let registry = self.registry.clone();

                handle_ids.push(id.clone());
                handles.push(tokio::task::spawn_blocking(move || {
                    let instance = registry
                        .create(&module_name, payload)
                        .unwrap_or_else(|| panic!("Module '{}' not found in registry", module_name));
                    instance.run();
                    instance.take_outputs()
                }));
            }

            for (handle, id) in handles.into_iter().zip(handle_ids) {
                let node_outputs = handle.await.unwrap();
                let output_map: HashMap<String, SharedValue> = node_outputs
                    .into_iter()
                    .map(|(k, v)| (k.to_string(), v))
                    .collect();
                outputs.insert(id, output_map);
            }
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
                        if let (Some(&from), Some(&to)) =
                            (index_map.get(dep_id), index_map.get(id))
                        {
                            graph.add_edge(from, to, property.clone());
                        }
                    }
                    Direction::Output => {
                        if let (Some(&from), Some(&to)) =
                            (index_map.get(id), index_map.get(dep_id))
                        {
                            graph.add_edge(from, to, prop.clone());
                        }
                    }
                },
                Value::Expression { value: expr, .. } => {
                    for (ref_id, _) in collect_expr_refs(expr) {
                        if let (Some(&from), Some(&to)) =
                            (index_map.get(ref_id), index_map.get(id))
                        {
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
    let topo = petgraph::algo::toposort(graph, None)
        .expect("Cycle detected in dependency graph");

    let mut node_levels: HashMap<NodeIndex, usize> = HashMap::new();

    for &idx in &topo {
        let max_dep_level = graph
            .neighbors_directed(idx, GraphDirection::Incoming)
            .filter_map(|dep| node_levels.get(&dep).copied())
            .max();

        let level = match max_dep_level {
            Some(l) => l + 1,
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
) -> Payload {
    let mut payload = HashMap::new();

    for (name, value) in &instruct.inputs {
        match value {
            Value::String {
                value, direction, ..
            } if *direction == Direction::Input => {
                payload.insert(name.clone(), Arc::new(value.clone()) as SharedValue);
            }
            Value::Numeric {
                value, direction, ..
            } if *direction == Direction::Input => {
                payload.insert(
                    name.clone(),
                    Arc::new(value.parse::<u32>().unwrap()) as SharedValue,
                );
            }
            Value::Expression {
                value: expr,
                direction,
            } if *direction == Direction::Input => {
                let result = eval_expr(expr, outputs, nodes);
                if result.fract() == 0.0 && result >= 0.0 && result <= u32::MAX as f64 {
                    payload.insert(name.clone(), Arc::new(result as u32) as SharedValue);
                } else {
                    payload.insert(name.clone(), Arc::new(result) as SharedValue);
                }
            }
            Value::Boolean {
                value, direction, ..
            } if *direction == Direction::Input => {
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
                    }
                }
            }
            _ => {}
        }
    }

    payload
}

/// Collect all node references from an expression tree.
fn collect_expr_refs(expr: &Expr) -> Vec<(&str, &str)> {
    match expr {
        Expr::Number(_) => vec![],
        Expr::Ref { identifier, property } => vec![(identifier.as_str(), property.as_str())],
        Expr::BinOp { left, right, .. } => {
            let mut refs = collect_expr_refs(left);
            refs.extend(collect_expr_refs(right));
            refs
        }
    }
}

/// Evaluate an expression tree, resolving node references from dependency outputs
/// or falling back to literal values from parsed node instructions.
fn eval_expr(
    expr: &Expr,
    outputs: &HashMap<Identifier, HashMap<String, SharedValue>>,
    nodes: &BTreeMap<Identifier, NodeInstruct>,
) -> f64 {
    match expr {
        Expr::Number(s) => s.parse::<f64>().unwrap(),
        Expr::Ref { identifier, property } => {
            // Try resolved outputs first (from executed nodes)
            if let Some(dep_outputs) = outputs.get(identifier) {
                if let Some(val) = dep_outputs.get(property) {
                    return shared_value_to_f64(val);
                }
            }
            // Fall back to literal inputs from parsed node (data-only nodes)
            if let Some(instruct) = nodes.get(identifier) {
                if let Some(value) = instruct.inputs.get(property) {
                    return match value {
                        Value::Numeric { value, .. } => value.parse::<f64>().unwrap(),
                        Value::Expression { value: inner, .. } => eval_expr(inner, outputs, nodes),
                        _ => panic!(
                            "Expression reference '{}::{}' is not numeric",
                            identifier, property
                        ),
                    };
                }
            }
            panic!(
                "Cannot resolve '{}::{}' in expression",
                identifier, property
            );
        }
        Expr::BinOp { op, left, right } => {
            let l = eval_expr(left, outputs, nodes);
            let r = eval_expr(right, outputs, nodes);
            match op {
                ExprOp::Add => l + r,
                ExprOp::Subtract => l - r,
                ExprOp::Multiply => l * r,
                ExprOp::Divide => l / r,
                ExprOp::Power => l.powf(r),
            }
        }
    }
}

fn shared_value_to_f64(val: &SharedValue) -> f64 {
    if let Some(v) = val.downcast_ref::<f64>() {
        *v
    } else if let Some(v) = val.downcast_ref::<u32>() {
        *v as f64
    } else if let Some(v) = val.downcast_ref::<i32>() {
        *v as f64
    } else if let Some(v) = val.downcast_ref::<i64>() {
        *v as f64
    } else if let Some(v) = val.downcast_ref::<f32>() {
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
        let nodes = NodeParser::parse(
            r#"
                a module_a { x <- 1 }
                b module_b { y <- 2 }
                c module_c { z <- 3 }
            "#,
        )
        .unwrap();

        let (graph, _) = build_dependency_graph(&nodes);
        let levels = compute_execution_levels(&graph);

        assert_eq!(levels.len(), 1, "all independent nodes should be at level 0");
        assert_eq!(levels[0].len(), 3);
    }

    #[test]
    fn test_build_graph_linear_chain() {
        let nodes = NodeParser::parse(
            r#"
                a module_a { x <- 1 }
                b module_b { y <- a::x }
                c module_c { z <- b::y }
            "#,
        )
        .unwrap();

        let (graph, index_map) = build_dependency_graph(&nodes);
        let levels = compute_execution_levels(&graph);

        assert_eq!(levels.len(), 3, "linear chain should have 3 levels");
        assert_eq!(levels[0].len(), 1);
        assert_eq!(levels[1].len(), 1);
        assert_eq!(levels[2].len(), 1);

        let a_level = levels.iter().position(|l| l.contains(&index_map[&String::from("a")])).unwrap();
        let b_level = levels.iter().position(|l| l.contains(&index_map[&String::from("b")])).unwrap();
        let c_level = levels.iter().position(|l| l.contains(&index_map[&String::from("c")])).unwrap();
        assert!(a_level < b_level);
        assert!(b_level < c_level);
    }

    #[test]
    fn test_build_graph_diamond() {
        let nodes = NodeParser::parse(
            r#"
                source module_s { x <- 1 }
                left module_l { y <- source::x }
                right module_r { z <- source::x }
                sink module_k { w <- left::y }
            "#,
        )
        .unwrap();

        let (graph, index_map) = build_dependency_graph(&nodes);
        let levels = compute_execution_levels(&graph);

        let source_level = levels.iter().position(|l| l.contains(&index_map[&String::from("source")])).unwrap();
        let left_level = levels.iter().position(|l| l.contains(&index_map[&String::from("left")])).unwrap();
        let right_level = levels.iter().position(|l| l.contains(&index_map[&String::from("right")])).unwrap();
        let sink_level = levels.iter().position(|l| l.contains(&index_map[&String::from("sink")])).unwrap();

        assert_eq!(source_level, 0);
        assert_eq!(left_level, right_level, "left and right should be at the same level");
        assert!(sink_level > left_level);
    }

    #[test]
    fn test_resolve_literal_inputs() {
        let nodes = NodeParser::parse(
            r#"
                a module_a {
                    s <- "hello"
                    n <- 42
                    b <- true
                }
            "#,
        )
        .unwrap();

        let instruct = &nodes[&String::from("a")];
        let outputs = HashMap::new();
        let payload = resolve_inputs(instruct, &outputs, &nodes);

        assert_eq!(payload.len(), 3);
        assert_eq!(
            *payload["s"].downcast_ref::<String>().unwrap(),
            "hello"
        );
        assert_eq!(*payload["n"].downcast_ref::<u32>().unwrap(), 42);
        assert_eq!(*payload["b"].downcast_ref::<bool>().unwrap(), true);
    }

    #[test]
    fn test_resolve_expression_literal_only() {
        let nodes = NodeParser::parse(
            r#"a module_a { x <- (10 + 20) }"#,
        )
        .unwrap();

        let instruct = &nodes[&String::from("a")];
        let outputs = HashMap::new();
        let payload = resolve_inputs(instruct, &outputs, &nodes);

        assert_eq!(*payload["x"].downcast_ref::<u32>().unwrap(), 30);
    }

    #[test]
    fn test_resolve_expression_with_ref() {
        let nodes = NodeParser::parse(
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

        let instruct = &nodes[&String::from("img")];
        let payload = resolve_inputs(instruct, &outputs, &nodes);

        assert_eq!(*payload["width"].downcast_ref::<u32>().unwrap(), 128);
    }

    #[test]
    fn test_expression_creates_dependency_edge() {
        let nodes = NodeParser::parse(
            r#"
                config constants { multiplier <- 4 }
                img resizer { width <- (32 * config::multiplier) }
            "#,
        )
        .unwrap();

        let (graph, index_map) = build_dependency_graph(&nodes);
        let levels = compute_execution_levels(&graph);

        let config_level = levels.iter().position(|l| l.contains(&index_map["config"])).unwrap();
        let img_level = levels.iter().position(|l| l.contains(&index_map["img"])).unwrap();

        assert!(config_level < img_level, "config must execute before img");
    }

    #[test]
    fn test_resolve_expression_multi_ref() {
        let nodes = NodeParser::parse(
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

        let instruct = &nodes[&String::from("c")];
        let payload = resolve_inputs(instruct, &outputs, &nodes);

        // 10 + 3 * 2 = 16 (respects operator precedence)
        assert_eq!(*payload["z"].downcast_ref::<u32>().unwrap(), 16);
    }
}