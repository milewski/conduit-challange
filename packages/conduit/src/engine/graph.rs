use crate::dsl::parser::{Direction, Identifier, NodeInstruct, StringPart, Value};
use petgraph::Direction as GraphDirection;
use petgraph::graph::{DiGraph, NodeIndex};
use std::collections::{BTreeMap, HashMap};

pub(super) fn build_dependency_graph(
    nodes: &BTreeMap<Identifier, NodeInstruct>,
    sequential_edges: &[(Identifier, Identifier)],
) -> (DiGraph<Identifier, String>, HashMap<Identifier, NodeIndex>) {
    let mut graph = DiGraph::new();
    let mut index_map = HashMap::new();

    for node_identifier in nodes.keys() {
        let node_index = graph.add_node(node_identifier.clone());
        index_map.insert(node_identifier.clone(), node_index);
    }

    for (node_identifier, node_instruct) in nodes {
        for (property_name, value) in &node_instruct.inputs {
            match value {
                Value::Relation {
                    identifier,
                    property,
                    direction,
                } => match direction {
                    Direction::Input => {
                        add_dependency_edge(&mut graph, &index_map, identifier, node_identifier, property);
                    }
                    Direction::Output => {
                        add_dependency_edge(&mut graph, &index_map, node_identifier, identifier, property);
                    }
                },
                Value::String { parts, .. } => {
                    for part in parts {
                        if let StringPart::Interpolation(expression) = part {
                            for (reference_identifier, _) in super::collect_expression_references(expression) {
                                add_dependency_edge(
                                    &mut graph,
                                    &index_map,
                                    reference_identifier,
                                    node_identifier,
                                    property_name,
                                );
                            }
                        }
                    }
                }
                Value::Expression { value, .. } => {
                    for (reference_identifier, _) in super::collect_expression_references(value) {
                        add_dependency_edge(
                            &mut graph,
                            &index_map,
                            reference_identifier,
                            node_identifier,
                            property_name,
                        );
                    }
                }
                Value::Tuple { values, .. } => {
                    for tuple_value in values {
                        collect_tuple_references(tuple_value, node_identifier, property_name, &index_map, &mut graph);
                    }
                }
                _ => {}
            }
        }
    }

    for (from_identifier, to_identifier) in sequential_edges {
        add_dependency_edge(&mut graph, &index_map, from_identifier, to_identifier, "__sequence__");
    }

    (graph, index_map)
}

pub(super) fn compute_execution_levels(graph: &DiGraph<Identifier, String>) -> Vec<Vec<NodeIndex>> {
    let topologically_sorted_nodes = petgraph::algo::toposort(graph, None).expect("Cycle detected in dependency graph");

    let mut node_levels: HashMap<NodeIndex, usize> = HashMap::new();

    for &node_index in &topologically_sorted_nodes {
        let max_dependency_level = graph
            .neighbors_directed(node_index, GraphDirection::Incoming)
            .filter_map(|dependency_index| node_levels.get(&dependency_index).copied())
            .max();

        let level = match max_dependency_level {
            Some(level) => level + 1,
            None => 0,
        };

        node_levels.insert(node_index, level);
    }

    let max_level = node_levels.values().max().copied().unwrap_or(0);
    let mut levels = vec![Vec::new(); max_level + 1];

    for &node_index in &topologically_sorted_nodes {
        levels[node_levels[&node_index]].push(node_index);
    }

    levels
}

fn add_dependency_edge(
    graph: &mut DiGraph<Identifier, String>,
    index_map: &HashMap<Identifier, NodeIndex>,
    from_identifier: &str,
    to_identifier: &str,
    property_name: &str,
) {
    let dependency_index = index_map
        .get(from_identifier)
        .or_else(|| index_map.get(&format!("__input_{}", from_identifier)));

    if let (Some(&from_index), Some(&to_index)) = (dependency_index, index_map.get(to_identifier)) {
        graph.add_edge(from_index, to_index, property_name.to_string());
    }
}

fn collect_tuple_references(
    value: &Value,
    current_identifier: &str,
    property_name: &str,
    index_map: &HashMap<Identifier, NodeIndex>,
    graph: &mut DiGraph<Identifier, String>,
) {
    match value {
        Value::Relation {
            identifier,
            property,
            direction,
        } => match direction {
            Direction::Input => add_dependency_edge(graph, index_map, identifier, current_identifier, property),
            Direction::Output => add_dependency_edge(graph, index_map, current_identifier, identifier, property),
        },
        Value::Tuple { values, .. } => {
            for nested_value in values {
                collect_tuple_references(nested_value, current_identifier, property_name, index_map, graph);
            }
        }
        Value::String { parts, .. } => {
            for part in parts {
                if let StringPart::Interpolation(expression) = part {
                    for (reference_identifier, _) in super::collect_expression_references(expression) {
                        add_dependency_edge(
                            graph,
                            index_map,
                            reference_identifier,
                            current_identifier,
                            property_name,
                        );
                    }
                }
            }
        }
        Value::Expression { value, .. } => {
            for (reference_identifier, _) in super::collect_expression_references(value) {
                add_dependency_edge(
                    graph,
                    index_map,
                    reference_identifier,
                    current_identifier,
                    property_name,
                );
            }
        }
        _ => {}
    }
}
