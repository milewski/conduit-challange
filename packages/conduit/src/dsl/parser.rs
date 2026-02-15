use crate::dsl::error::ParserError;
use crate::node::SharedValue;
use pest::Parser;
use pest::iterators::{Pair, Pairs};
use pest::pratt_parser::{Assoc, Op, PrattParser};
use pest_derive::Parser;
use std::any::TypeId;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::ops::Index;
use uuid::Uuid;

#[derive(Debug, PartialEq, Clone, Eq, Hash)]
pub enum Expression {
    Number(String),
    String(String),
    Reference {
        identifier: String,
        property: String,
    },
    BinaryOperation {
        operation: Operation,
        left: Box<Expression>,
        right: Box<Expression>,
    },
}

#[derive(Debug, PartialEq, Clone, Copy, Eq, Hash)]
pub enum Operation {
    Add,
    Subtract,
    Multiply,
    Divide,
    Power,
}

#[derive(Debug, PartialEq, Clone, Eq, Hash)]
pub enum StringPart {
    Literal(String),
    Interpolation(Expression),
}

#[derive(Debug, PartialEq, Clone, Eq, Hash)]
pub enum Value {
    String {
        direction: Direction,
        parts: Vec<StringPart>,
    },
    Numeric {
        direction: Direction,
        value: String,
    },
    Boolean {
        direction: Direction,
        value: bool,
    },
    Expression {
        direction: Direction,
        value: Expression,
    },
    Relation {
        identifier: Identifier,
        direction: Direction,
        property: String,
    },
    Tuple {
        direction: Direction,
        values: Vec<Value>,
    },
}

#[derive(Debug, PartialEq, Clone, Eq, Hash)]
pub enum CallbackAssignmentOperation {
    Assign,
    Append,
}

#[derive(Debug, PartialEq, Clone, Eq, Hash)]
pub struct CallbackAssignment {
    pub identifier: Identifier,
    pub property: Property,
    pub operation: CallbackAssignmentOperation,
    pub value: Value,
}

#[derive(Debug, PartialEq, Clone, Eq, Hash)]
pub enum EventCallback {
    Value(Value),
    PipedValue(Value),
    Assignment(CallbackAssignment),
    Block(Vec<EventCallback>),
}

impl Value {
    pub fn direction(&self) -> Direction {
        match self {
            Value::String { direction, .. }
            | Value::Numeric { direction, .. }
            | Value::Boolean { direction, .. }
            | Value::Expression { direction, .. }
            | Value::Relation { direction, .. }
            | Value::Tuple { direction, .. } => *direction,
        }
    }
}

#[derive(Debug, PartialEq, Clone, Copy, Eq, Hash)]
pub enum Direction {
    Input,
    Output,
}

impl Direction {
    pub fn reverse(self) -> Self {
        match self {
            Direction::Input => Direction::Output,
            Direction::Output => Direction::Input,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Direction::Input => "input",
            Direction::Output => "output",
        }
    }
}

impl From<Pair<'_, Rule>> for Direction {
    fn from(value: Pair<'_, Rule>) -> Self {
        match value.as_str() {
            "<-" => Direction::Input,
            "<<-" => Direction::Input,
            "->" => Direction::Output,
            _ => unreachable!(),
        }
    }
}

#[derive(Parser)]
#[grammar = "schema.pest"]
struct Schema;

pub type Identifier = String;
pub type Property = String;
pub const EVENT_PAYLOAD_IDENTIFIER: &str = "__event_payload__";
pub const EVENT_ALIAS_IDENTIFIER_PREFIX: &str = "__event_alias__";

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct NodeInstruct {
    pub identifier: Identifier,
    pub module: String,
    pub inputs: BTreeMap<Property, Value>,
}

impl NodeInstruct {
    pub fn new(identifier: Option<&str>, module: &str) -> Self {
        NodeInstruct {
            identifier: identifier
                .map(|identifier| identifier.to_string())
                .unwrap_or_else(|| Uuid::new_v4().to_string()),
            module: module.to_string(),
            inputs: BTreeMap::default(),
        }
    }
}

#[derive(Debug, PartialEq, Clone, Eq)]
pub struct ParsedWorkflow {
    pub nodes: BTreeMap<Identifier, NodeInstruct>,
    pub inputs: BTreeMap<Identifier, Option<Value>>,
    pub event_handlers: BTreeMap<Identifier, BTreeMap<String, Vec<EventCallback>>>,
    pub event_callback_nodes: HashSet<Identifier>,
    pub sequential_edges: Vec<(Identifier, Identifier)>,
}

#[derive(Debug, Default)]
struct Visitor {
    nodes: BTreeMap<Identifier, NodeInstruct>,
    inputs: BTreeMap<Identifier, Option<Value>>,
    event_handlers: BTreeMap<Identifier, BTreeMap<String, Vec<EventCallback>>>,
    event_callback_nodes: HashSet<Identifier>,
    sequential_edges: Vec<(Identifier, Identifier)>,
    external_inputs: HashMap<Identifier, SharedValue>,
    scope: HashMap<Identifier, Value>,
    aliases: HashMap<Identifier, Identifier>,
    suffix: String,
}

fn expression_parser() -> PrattParser<Rule> {
    PrattParser::new()
        .op(Op::infix(Rule::add, Assoc::Left) | Op::infix(Rule::subtract, Assoc::Left))
        .op(Op::infix(Rule::multiply, Assoc::Left) | Op::infix(Rule::divide, Assoc::Left))
        .op(Op::infix(Rule::power, Assoc::Right))
}

impl Visitor {
    fn visit_sequence_group(&mut self, sequence_group: Pair<Rule>) -> Result<(), ParserError> {
        assert_eq!(sequence_group.as_rule(), Rule::sequence_group);

        let mut sequence_node_identifiers = Vec::new();

        for sequence_item in sequence_group.into_inner() {
            match sequence_item.as_rule() {
                Rule::node | Rule::anonymous_node => {
                    let (node, _) = self.visit_node(sequence_item)?;
                    sequence_node_identifiers.push(node.identifier.clone());
                }
                _ => unreachable!("Unexpected rule in sequence group: {:?}", sequence_item.as_rule()),
            }
        }

        for sequence_pair in sequence_node_identifiers.windows(2) {
            let from_identifier = sequence_pair[0].clone();
            let to_identifier = sequence_pair[1].clone();
            self.sequential_edges.push((from_identifier, to_identifier));
        }

        Ok(())
    }

    fn ensure_callback_node(&mut self, callback_name: &str) -> Result<Identifier, ParserError> {
        if let Some(existing_alias) = self.aliases.get(callback_name) {
            return Ok(existing_alias.clone());
        }

        if self.nodes.contains_key(callback_name) {
            return Ok(callback_name.to_string());
        }

        let callback_node = NodeInstruct::new(Some(callback_name), callback_name);

        if self.nodes.contains_key(&callback_node.identifier) {
            return Err(ParserError::DuplicatedNode {
                identifier: callback_node.identifier,
            });
        }

        let callback_identifier = callback_node.identifier.clone();
        self.event_callback_nodes.insert(callback_identifier.clone());
        self.nodes.insert(callback_identifier.clone(), callback_node);
        Ok(callback_identifier)
    }

    fn parse_callback_assignment(
        &mut self,
        callback_assignment: Pair<Rule>,
    ) -> Result<CallbackAssignment, ParserError> {
        assert_eq!(callback_assignment.as_rule(), Rule::callback_assignment);
        let mut pairs = callback_assignment.into_inner();
        let relation_pair = pairs.next().unwrap_or_else(|| unreachable!());
        let operation_pair = pairs.next().unwrap_or_else(|| unreachable!());
        let value_pair = pairs.next().unwrap_or_else(|| unreachable!());

        let mut relation_parts = relation_pair.into_inner();
        let identifier_pair = relation_parts.next().unwrap_or_else(|| unreachable!());
        let property_pair = relation_parts.next().unwrap_or_else(|| unreachable!());

        let identifier_string = identifier_pair.as_str();
        let resolved_identifier = self
            .aliases
            .get(identifier_string)
            .cloned()
            .unwrap_or_else(|| identifier_string.to_string());

        let callback_value = self.visit_value(
            value_pair.into_inner().next().unwrap_or_else(|| unreachable!()),
            Direction::Input,
        )?;

        Ok(CallbackAssignment {
            identifier: resolved_identifier,
            property: property_pair.as_str().to_string(),
            operation: match operation_pair.as_str() {
                "<-" => CallbackAssignmentOperation::Assign,
                "<<-" => CallbackAssignmentOperation::Append,
                _ => unreachable!(),
            },
            value: callback_value,
        })
    }

    fn parse_event_callback(
        &mut self,
        event_callback: Pair<Rule>,
        is_piped: bool,
    ) -> Result<EventCallback, ParserError> {
        let callback_inner = if event_callback.as_rule() == Rule::event_callback {
            event_callback.into_inner().next().unwrap_or_else(|| unreachable!())
        } else {
            event_callback
        };

        match callback_inner.as_rule() {
            Rule::value => {
                let callback_value_pair = callback_inner.into_inner().next().unwrap_or_else(|| unreachable!());
                let callback_value_rule = callback_value_pair.as_rule();
                let existing_node_identifiers: HashSet<Identifier> =
                    if matches!(callback_value_rule, Rule::node | Rule::anonymous_node) {
                        self.nodes.keys().cloned().collect()
                    } else {
                        HashSet::new()
                    };
                let mut callback_value = self.visit_value(callback_value_pair, Direction::Input)?;

                if let Value::Relation {
                    identifier,
                    direction,
                    property,
                } = &mut callback_value
                {
                    if callback_value_rule == Rule::identifier && *direction == Direction::Input && property == "output"
                    {
                        let _ = self.ensure_callback_node(identifier)?;
                        *property = "input".to_string();
                    }

                    if callback_value_rule == Rule::relation
                        && !self.nodes.contains_key(identifier)
                        && !self.aliases.contains_key(identifier)
                    {
                        let _ = self.ensure_callback_node(identifier)?;
                    }

                    if matches!(callback_value_rule, Rule::node | Rule::anonymous_node) {
                        for node_identifier in self.nodes.keys() {
                            if !existing_node_identifiers.contains(node_identifier) {
                                self.event_callback_nodes.insert(node_identifier.clone());
                            }
                        }
                        if *direction == Direction::Input && property == "output" {
                            *property = "input".to_string();
                        }
                    }
                }

                if is_piped {
                    Ok(EventCallback::PipedValue(callback_value))
                } else {
                    Ok(EventCallback::Value(callback_value))
                }
            }
            Rule::callback_block => {
                let mut callbacks = Vec::new();

                for callback_statement in callback_inner.into_inner() {
                    let statement = callback_statement.into_inner().next().unwrap_or_else(|| unreachable!());
                    match statement.as_rule() {
                        Rule::callback_assignment => {
                            callbacks.push(EventCallback::Assignment(self.parse_callback_assignment(statement)?));
                        }
                        Rule::node | Rule::anonymous_node => {
                            let existing_node_identifiers: HashSet<Identifier> = self.nodes.keys().cloned().collect();
                            let mut node_value = self.visit_value(statement, Direction::Input)?;
                            if let Value::Relation {
                                direction, property, ..
                            } = &mut node_value
                            {
                                if *direction == Direction::Input && property == "output" {
                                    *property = "input".to_string();
                                }
                                for node_identifier in self.nodes.keys() {
                                    if !existing_node_identifiers.contains(node_identifier) {
                                        self.event_callback_nodes.insert(node_identifier.clone());
                                    }
                                }
                            }
                            callbacks.push(EventCallback::Value(node_value));
                        }
                        _ => unreachable!("Unexpected callback statement: {:?}", statement.as_rule()),
                    }
                }

                Ok(EventCallback::Block(callbacks))
            }
            _ => unreachable!("Unexpected callback rule: {:?}", callback_inner.as_rule()),
        }
    }

    fn visit_event_handler(&mut self, node: &mut NodeInstruct, event_handler: Pair<Rule>) -> Result<(), ParserError> {
        assert_eq!(event_handler.as_rule(), Rule::event_handler);

        let mut pairs = event_handler.into_inner();
        let event_names_pair = pairs.next().unwrap_or_else(|| unreachable!());
        let event_names: Vec<String> = match event_names_pair.as_rule() {
            Rule::event_names => event_names_pair
                .into_inner()
                .map(|event_name_pair| event_name_pair.as_str().to_string())
                .collect(),
            _ => unreachable!(),
        };

        let handler_pair = pairs.next().unwrap_or_else(|| unreachable!());
        let (event_payload_alias, callback_pair, is_piped) = match handler_pair.as_rule() {
            Rule::event_handler_pipe => {
                let callback_pair = handler_pair.into_inner().next().unwrap_or_else(|| unreachable!());
                (None, callback_pair, true)
            }
            Rule::event_handler_block => {
                let mut handler_pairs = handler_pair.into_inner();
                let first_pair = handler_pairs.next().unwrap_or_else(|| unreachable!());
                if first_pair.as_rule() == Rule::event_payload_alias {
                    let callback_block = handler_pairs.next().unwrap_or_else(|| unreachable!());
                    (Some(first_pair.as_str().to_string()), callback_block, false)
                } else {
                    (None, first_pair, false)
                }
            }
            _ => unreachable!(),
        };

        let default_event_payload_alias = if event_names.len() == 1 {
            event_names.first().cloned()
        } else {
            None
        };

        let payload_alias_to_use = event_payload_alias.or(default_event_payload_alias);
        let alias_storage_identifier = payload_alias_to_use
            .as_ref()
            .map(|payload_alias| format!("{}{}_{}", EVENT_ALIAS_IDENTIFIER_PREFIX, payload_alias, Uuid::new_v4()));

        let previous_scope_value = if let (Some(payload_alias), Some(alias_storage_identifier)) =
            (payload_alias_to_use.as_ref(), alias_storage_identifier.as_ref())
        {
            self.scope.insert(
                payload_alias.clone(),
                Value::Relation {
                    identifier: alias_storage_identifier.clone(),
                    direction: Direction::Input,
                    property: "output".to_string(),
                },
            )
        } else {
            None
        };

        let mut parsed_callback = self.parse_event_callback(callback_pair, is_piped)?;

        if let Some(alias_storage_identifier) = alias_storage_identifier {
            let alias_capture_assignment = EventCallback::Assignment(CallbackAssignment {
                identifier: alias_storage_identifier,
                property: "output".to_string(),
                operation: CallbackAssignmentOperation::Assign,
                value: Value::Relation {
                    identifier: EVENT_PAYLOAD_IDENTIFIER.to_string(),
                    direction: Direction::Input,
                    property: "output".to_string(),
                },
            });

            parsed_callback = match parsed_callback {
                EventCallback::Block(mut callbacks) => {
                    callbacks.insert(0, alias_capture_assignment);
                    EventCallback::Block(callbacks)
                }
                callback => EventCallback::Block(vec![alias_capture_assignment, callback]),
            };
        }

        if let Some(payload_alias) = payload_alias_to_use {
            if let Some(previous_scope_value) = previous_scope_value {
                self.scope.insert(payload_alias, previous_scope_value);
            } else {
                self.scope.remove(&payload_alias);
            }
        }

        for event_name in event_names {
            self.event_handlers
                .entry(node.identifier.clone())
                .or_default()
                .entry(event_name)
                .or_default()
                .push(parsed_callback.clone());
        }

        Ok(())
    }

    pub fn visit_for_loop(&mut self, pair: Pair<Rule>) -> Result<(), ParserError> {
        assert_eq!(pair.as_rule(), Rule::for_loop);

        let mut pairs = pair.into_inner();

        let identifier = pairs.next().unwrap().as_str().to_string();
        let iterable_pair = pairs.next().unwrap();
        let loop_body_pair = pairs.next().unwrap();

        let inner_iterable = iterable_pair.into_inner().next().unwrap();

        let values: Vec<Value> = match inner_iterable.as_rule() {
            Rule::range => {
                let mut range_pairs = inner_iterable.into_inner();
                let start_pair = range_pairs.next().unwrap();
                let end_pair = range_pairs.next().unwrap();

                let start = self.resolve_range_bound(start_pair)?;
                let end = self.resolve_range_bound(end_pair)?;

                (start..end)
                    .map(|index| Value::Numeric {
                        direction: Direction::Input,
                        value: index.to_string(),
                    })
                    .collect()
            }
            Rule::identifier => {
                let name = inner_iterable.as_str();
                let value = if let Some(value) = self.scope.get(name) {
                    Some(value.clone())
                } else if let Some(Some(value)) = self.inputs.get(name) {
                    Some(value.clone())
                } else if let Some(external_val) = self.external_inputs.get(name) {
                    convert_shared_value_to_parser_value(external_val)
                } else {
                    None
                };

                match value {
                    Some(Value::Tuple { values, .. }) => values,
                    Some(_) => return Err(ParserError::ConstantNotFound(format!("{} is not an array", name))),
                    None => return Err(ParserError::ConstantNotFound(name.to_string())),
                }
            }
            Rule::relation => {
                let mut pairs = inner_iterable.into_inner();
                let id_str = pairs.next().unwrap().as_str();
                let prop_str = pairs.next().unwrap().as_str();

                let resolved_id = self.aliases.get(id_str).map(|s| s.as_str()).unwrap_or(id_str);

                let value = if let Some(node) = self.nodes.get(resolved_id) {
                    node.inputs.get(prop_str).cloned()
                } else {
                    None
                };

                match value {
                    Some(Value::Tuple { values, .. }) => values,
                    Some(_) => {
                        return Err(ParserError::ConstantNotFound(format!(
                            "{}::{} is not an array",
                            id_str, prop_str
                        )));
                    }
                    None => return Err(ParserError::ConstantNotFound(format!("{}::{}", id_str, prop_str))),
                }
            }
            Rule::array => {
                let mut values = Vec::new();
                for value_pair in inner_iterable.into_inner() {
                    values.push(self.visit_value(value_pair, Direction::Input)?);
                }
                values
            }
            _ => unreachable!(),
        };

        let old_suffix = self.suffix.clone();

        for (index, value) in values.into_iter().enumerate() {
            if identifier != "_" {
                self.scope.insert(identifier.clone(), value);
            }

            // Append iteration to suffix to ensure unique node IDs inside loop
            self.suffix = format!("{}{}", old_suffix, index);

            for child in loop_body_pair.clone().into_inner() {
                match child.as_rule() {
                    Rule::for_loop => self.visit_for_loop(child)?,
                    Rule::node | Rule::anonymous_node => {
                        self.visit_node(child)?;
                    }
                    Rule::sequence_group => self.visit_sequence_group(child)?,
                    _ => unreachable!("Unexpected rule in loop body: {:?}", child.as_rule()),
                }
            }
        }

        self.suffix = old_suffix;

        if identifier != "_" {
            self.scope.remove(&identifier);
        }

        Ok(())
    }

    pub fn visit_node<'a>(
        &'a mut self,
        node: Pair<'a, Rule>,
    ) -> Result<(&'a NodeInstruct, Option<Pair<'a, Rule>>), ParserError> {
        let kind = node.as_rule();
        let mut pairs = node.into_inner();

        let (identifier, module, body_or_shorthand) = match kind {
            Rule::node => (
                pairs.next().map(|pair| pair.as_str()),
                pairs.next().unwrap_or_else(|| unreachable!()),
                pairs.next().unwrap_or_else(|| unreachable!()),
            ),
            Rule::anonymous_node => (
                None,
                pairs.next().unwrap_or_else(|| unreachable!()),
                pairs.next().unwrap_or_else(|| unreachable!()),
            ),
            _ => unreachable!(),
        };

        let mut module_pairs = module.into_inner();
        let (module_identifier, module_property) = (
            module_pairs.next().unwrap_or_else(|| unreachable!()),
            module_pairs.next(),
        );

        // Check if this is an "assignment" (anonymous node where module looks like "existing_alias::property")
        // But be careful: module_identifier is just a string.
        let is_assignment = if identifier.is_none() && module_property.is_some() {
            let alias_name = module_identifier.as_str();
            // Check if alias exists (either global or local alias)
            self.aliases.contains_key(alias_name) || self.nodes.contains_key(alias_name)
        } else {
            false
        };

        let mut node = if is_assignment {
            let alias_name = module_identifier.as_str();
            // Resolve the current version of the node
            let resolved_id = self.aliases.get(alias_name).map(|s| s.as_str()).unwrap_or(alias_name);

            // We need to fetch the original module type of this node to create a new version
            // But we don't store module type in `aliases`. We must look up the node.
            let original_node = self
                .nodes
                .get(resolved_id)
                .ok_or_else(|| ParserError::ModuleNotDefined {
                    identifier: resolved_id.to_string(),
                })?;

            let original_module = original_node.module.clone();

            // Create a new version
            let new_id = if self.suffix.is_empty() {
                Uuid::new_v4().to_string()
            } else {
                format!("{}_{}", alias_name, self.suffix)
            };

            let mut new_node = NodeInstruct::new(Some(&new_id), &original_module);
            new_node.inputs = original_node.inputs.clone();

            new_node
        } else {
            // Normal node definition
            let id = if let Some(id_str) = identifier {
                if !self.suffix.is_empty() {
                    let new_id = format!("{}_{}", id_str, self.suffix);
                    self.aliases.insert(id_str.to_string(), new_id.clone());
                    new_id
                } else {
                    id_str.to_string()
                }
            } else {
                // Anonymous
                Uuid::new_v4().to_string()
            };

            NodeInstruct::new(Some(&id), module_identifier.as_str())
        };

        // Re-check duplicated node if we are not in assignment mode (or if explicit ID collide)
        if !is_assignment && self.nodes.contains_key(&node.identifier) {
            return Err(ParserError::DuplicatedNode {
                identifier: node.identifier,
            });
        }

        if is_assignment {
            // For assignment, we need to manually handle the shorthand/body to target specific property
            // Previous logic: `visit_shorthand` puts into direction-based prop.
            // We want `store::counter <- val` -> put val into `counter`.

            let target_prop = module_property.as_ref().unwrap().as_str().to_string();

            match body_or_shorthand.as_rule() {
                Rule::shorthand => {
                    let mut pairs = body_or_shorthand.into_inner();
                    let direction_pair = pairs.next().unwrap_or_else(|| unreachable!());
                    let pair = pairs.next().unwrap().into_inner().next().unwrap();
                    let value = self.visit_value(pair, Direction::Input)?;
                    if direction_pair.as_str() == "<<-" {
                        let mut values = match node.inputs.get(&target_prop).cloned() {
                            Some(Value::Tuple { values, .. }) => values,
                            Some(_) => {
                                return Err(ParserError::AppendToNonArray {
                                    identifier: module_identifier.as_str().to_string(),
                                    property: target_prop,
                                });
                            }
                            None => Vec::new(),
                        };
                        values.push(value);
                        node.inputs.insert(
                            target_prop,
                            Value::Tuple {
                                direction: Direction::Input,
                                values,
                            },
                        );
                    } else {
                        node.inputs.insert(target_prop, value);
                    }
                }
                Rule::body => self.visit_body(&mut node, body_or_shorthand)?, // Body allows multiple params
                _ => unreachable!(),
            }

            // Update alias to point to this new version
            let alias_name = module_identifier.as_str();
            self.aliases.insert(alias_name.to_string(), node.identifier.clone());
        } else {
            match body_or_shorthand.as_rule() {
                Rule::body => self.visit_body(&mut node, body_or_shorthand)?,
                Rule::shorthand => self.visit_shorthand(&mut node, body_or_shorthand)?,
                _ => unreachable!(),
            }
        }

        let identifier = node.identifier.clone();

        self.nodes.insert(identifier.clone(), node);

        // Return logic is tricky: return reference to inserted node.
        // Also returns `module_property` which is used for anonymous node linking (implicit output).
        // If assignment, we probably don't need to link output?
        Ok((
            self.nodes.index(&identifier),
            if is_assignment { None } else { module_property },
        ))
    }

    pub fn visit_shorthand(&mut self, node: &mut NodeInstruct, shorthand: Pair<Rule>) -> Result<(), ParserError> {
        assert_eq!(shorthand.as_rule(), Rule::shorthand);

        let mut pairs = shorthand.into_inner();
        let direction_pair = pairs.next().unwrap_or_else(|| unreachable!());
        let value_pair = pairs.next().unwrap_or_else(|| unreachable!());

        let direction: Direction = direction_pair.into();
        let property = direction.as_str().to_string();

        let inner = value_pair.into_inner().next().unwrap_or_else(|| unreachable!());
        let value = self.visit_value(inner, direction)?;

        node.inputs.insert(property, value);

        Ok(())
    }

    pub fn visit_body(&mut self, node: &mut NodeInstruct, body: Pair<Rule>) -> Result<(), ParserError> {
        assert_eq!(body.as_rule(), Rule::body);

        for pair in body.into_inner() {
            assert_eq!(pair.as_rule(), Rule::body_item);
            let body_item = pair.into_inner().next().unwrap_or_else(|| unreachable!());

            match body_item.as_rule() {
                Rule::parameter => {
                    let mut pairs = body_item.into_inner();
                    let first = pairs.next().unwrap_or_else(|| unreachable!());

                    let (property_names, direction_pair, value_pair) = if first.as_rule() == Rule::parameter_target {
                        let mut target_pairs = first.into_inner();
                        let target = target_pairs.next().unwrap_or_else(|| unreachable!());
                        let direction = pairs.next().unwrap_or_else(|| unreachable!());
                        let value = pairs.next().unwrap_or_else(|| unreachable!());
                        let names = match target.as_rule() {
                            Rule::property => vec![target.as_str().to_string()],
                            Rule::property_list => target
                                .into_inner()
                                .map(|property| property.as_str().to_string())
                                .collect(),
                            _ => unreachable!("Unexpected parameter target: {:?}", target.as_rule()),
                        };
                        (names, direction, value)
                    } else if first.as_rule() == Rule::direction {
                        let direction = first;
                        let value = pairs.next().unwrap_or_else(|| unreachable!());
                        let direction_string = direction.as_str();
                        let property_names = match direction_string {
                            "<-" | "<<-" => {
                                if node.module == "_" {
                                    vec!["output".to_string()]
                                } else {
                                    vec!["input".to_string()]
                                }
                            }
                            "->" => vec!["output".to_string()],
                            _ => unreachable!(),
                        };
                        (property_names, direction, value)
                    } else {
                        unreachable!("Unexpected rule in parameter: {:?}", first.as_rule());
                    };

                    assert_eq!(direction_pair.as_rule(), Rule::direction);
                    assert_eq!(value_pair.as_rule(), Rule::value);

                    let inner = value_pair.into_inner().next().unwrap_or_else(|| unreachable!());
                    let direction: Direction = direction_pair.into();
                    let value = self.visit_value(inner, direction)?;

                    for property_name in property_names {
                        node.inputs.insert(property_name, value.clone());
                    }
                }
                Rule::event_handler => {
                    self.visit_event_handler(node, body_item)?;
                }
                _ => unreachable!("Unexpected rule in node body: {:?}", body_item.as_rule()),
            }
        }

        Ok(())
    }

    pub fn visit_input_def(&mut self, pair: Pair<Rule>) -> Result<(), ParserError> {
        assert_eq!(pair.as_rule(), Rule::input_def);
        let mut input_names = Vec::new();
        let mut optional_value: Option<Value> = None;

        for input_def_part in pair.into_inner() {
            match input_def_part.as_rule() {
                Rule::input_identifiers => {
                    for identifier_pair in input_def_part.into_inner() {
                        assert_eq!(identifier_pair.as_rule(), Rule::identifier);
                        input_names.push(identifier_pair.as_str().to_string());
                    }
                }
                Rule::value => {
                    let inner_value = input_def_part.into_inner().next().unwrap_or_else(|| unreachable!());
                    optional_value = Some(self.visit_value(inner_value, Direction::Input)?);
                }
                _ => unreachable!(),
            }
        }

        for input_name in input_names {
            let resolved_value = if let Some(value) = &optional_value {
                Some(value.clone())
            } else if let Some(external_value) = self.external_inputs.get(&input_name) {
                convert_shared_value_to_parser_value(external_value)
            } else {
                None
            };

            if self.inputs.contains_key(&input_name) {
                return Err(ParserError::DuplicatedNode { identifier: input_name });
            }
            self.inputs.insert(input_name, resolved_value);
        }

        Ok(())
    }

    fn resolve_range_bound(&self, pair: Pair<Rule>) -> Result<i32, ParserError> {
        let inner = pair.into_inner().next().unwrap();
        match inner.as_rule() {
            Rule::number => inner.as_str().parse().map_err(|_| ParserError::InvalidNumber),
            Rule::interpolation => {
                let content = inner.into_inner().next().unwrap().as_str();
                let mut pairs =
                    Schema::parse(Rule::interpolation_expression, content).map_err(|e| ParserError::from(e))?;
                let expr_pair = pairs.next().unwrap().into_inner().next().unwrap();
                self.evaluate_expression_constant(expr_pair)
            }
            Rule::relation => self.evaluate_relation_constant(inner),
            Rule::identifier => self.evaluate_identifier_constant(inner),
            _ => unreachable!(),
        }
    }

    fn evaluate_expression_constant(&self, pair: Pair<Rule>) -> Result<i32, ParserError> {
        let pairs = pair.into_inner();

        expression_parser()
            .map_primary(|primary| -> Result<i32, ParserError> {
                match primary.as_rule() {
                    Rule::number => primary.as_str().parse().map_err(|_| ParserError::InvalidNumber),
                    Rule::identifier => self.evaluate_identifier_constant(primary),
                    Rule::relation => self.evaluate_relation_constant(primary),
                    Rule::expression => self.evaluate_expression_constant(primary),
                    _ => Err(ParserError::NonConstantExpression),
                }
            })
            .map_infix(|left, operation, right| {
                let left = left?;
                let right = right?;

                match operation.as_rule() {
                    Rule::add => Ok(left + right),
                    Rule::subtract => Ok(left - right),
                    Rule::multiply => Ok(left * right),
                    Rule::divide => {
                        if right == 0 {
                            Err(ParserError::DivisionByZero)
                        } else {
                            Ok(left / right)
                        }
                    }
                    Rule::power => {
                        if right < 0 {
                            Err(ParserError::NegativeExponent)
                        } else {
                            Ok(left.pow(right as u32))
                        }
                    }
                    _ => unreachable!(),
                }
            })
            .parse(pairs)
    }

    fn evaluate_identifier_constant(&self, pair: Pair<Rule>) -> Result<i32, ParserError> {
        let name = pair.as_str();

        if let Some(value) = self.scope.get(name) {
            return self.value_to_int(value);
        }

        if let Some(Some(value)) = self.inputs.get(name) {
            return self.value_to_int(value);
        }

        Err(ParserError::ConstantNotFound(name.to_string()))
    }

    fn evaluate_relation_constant(&self, pair: Pair<Rule>) -> Result<i32, ParserError> {
        let mut pairs = pair.into_inner();
        let identifier = pairs.next().unwrap().as_str();
        let property = pairs.next().unwrap().as_str();

        let resolved_id = self
            .aliases
            .get(identifier)
            .map(|alias| alias.as_str())
            .unwrap_or(identifier);

        if let Some(node) = self.nodes.get(resolved_id) {
            if let Some(value) = node.inputs.get(property) {
                return self.value_to_int(value);
            }
        }

        Err(ParserError::ConstantNotFound(format!("{}::{}", identifier, property)))
    }

    fn value_to_int(&self, value: &Value) -> Result<i32, ParserError> {
        match value {
            Value::Numeric { value, .. } => value.parse().map_err(|_| ParserError::InvalidNumber),
            _ => Err(ParserError::NonNumericValue),
        }
    }

    pub fn visit_pipeline_result(&mut self, pair: Pair<Rule>) -> Result<(), ParserError> {
        assert_eq!(pair.as_rule(), Rule::pipeline_result);
        let direction = Direction::Input;

        for value_pair in pair.into_inner() {
            let inner = value_pair.into_inner().next().unwrap_or_else(|| unreachable!());
            let value = self.visit_value(inner, direction)?;

            self.append_pipeline_result_value(value, direction);
        }

        Ok(())
    }

    fn append_pipeline_result_value(&mut self, value: Value, direction: Direction) {
        if let Some(node) = self.nodes.get_mut(PIPELINE_RESULT_ID) {
            if let Some(existing_value) = node.inputs.get_mut("input") {
                match existing_value {
                    Value::Tuple { values, .. } => {
                        values.push(value);
                    }
                    _ => {
                        let old_value = existing_value.clone();
                        *existing_value = Value::Tuple {
                            direction,
                            values: vec![old_value, value],
                        };
                    }
                }
            } else {
                node.inputs.insert("input".to_string(), value);
            }
        } else {
            let mut node = NodeInstruct::new(Some(PIPELINE_RESULT_ID), "_");
            node.inputs.insert("input".to_string(), value);
            self.nodes.insert(PIPELINE_RESULT_ID.to_string(), node);
        }
    }

    fn visit_value(&mut self, pair: Pair<Rule>, direction: Direction) -> Result<Value, ParserError> {
        let pair = if pair.as_rule() == Rule::value {
            pair.into_inner().next().unwrap()
        } else {
            pair
        };

        match pair.as_rule() {
            Rule::expression => Ok(Value::Expression {
                direction,
                value: self.visit_expression(pair.into_inner())?,
            }),
            Rule::number => Ok(Value::Numeric {
                direction,
                value: pair.as_str().to_string(),
            }),
            Rule::string => {
                let mut parts = Vec::new();
                for inner in pair.into_inner() {
                    match inner.as_rule() {
                        Rule::string_content => {
                            parts.push(StringPart::Literal(inner.as_str().to_string()));
                        }
                        Rule::interpolation => {
                            let inner_pair = inner.into_inner().next().unwrap();
                            let inner_str = inner_pair.as_str();

                            let mut pairs = Schema::parse(Rule::interpolation_expression, inner_str)
                                .map_err(|e| ParserError::from(e))?;

                            let expression_pair = pairs.next().unwrap().into_inner().next().unwrap();
                            let expression = self.visit_expression(expression_pair.into_inner())?;

                            parts.push(StringPart::Interpolation(expression));
                        }
                        _ => unreachable!("Unexpected rule in string: {:?}", inner.as_rule()),
                    }
                }
                Ok(Value::String { direction, parts })
            }
            Rule::boolean => Ok(Value::Boolean {
                direction,
                value: match pair.as_str() {
                    "true" => true,
                    "false" => false,
                    _ => unreachable!(),
                },
            }),
            Rule::identifier => {
                let identifier_str = pair.as_str();
                if let Some(value) = self.scope.get(identifier_str) {
                    Ok(value.clone())
                } else {
                    let identifier = self
                        .aliases
                        .get(identifier_str)
                        .cloned()
                        .unwrap_or_else(|| identifier_str.to_string());
                    Ok(Value::Relation {
                        direction,
                        identifier,
                        property: direction.reverse().as_str().to_string(),
                    })
                }
            }
            Rule::node | Rule::anonymous_node => {
                let (node, property) = self.visit_node(pair)?;

                Ok(Value::Relation {
                    direction,
                    property: property
                        .map(|property| property.as_str().to_string())
                        .unwrap_or_else(|| direction.reverse().as_str().to_string()),
                    identifier: node.identifier.clone(),
                })
            }
            Rule::relation => {
                let mut pairs = pair.into_inner();

                let (identifier_pair, related_property) = (
                    pairs.next().unwrap_or_else(|| unreachable!()),
                    pairs.next().unwrap_or_else(|| unreachable!()),
                );

                assert_eq!(identifier_pair.as_rule(), Rule::identifier);
                assert_eq!(related_property.as_rule(), Rule::property);

                let identifier_str = identifier_pair.as_str();
                let identifier = self
                    .aliases
                    .get(identifier_str)
                    .cloned()
                    .unwrap_or_else(|| identifier_str.to_string());

                Ok(Value::Relation {
                    direction,
                    identifier,
                    property: related_property.as_str().to_string(),
                })
            }
            Rule::array => {
                let mut values = Vec::new();
                for inner in pair.into_inner() {
                    values.push(self.visit_value(inner, direction)?);
                }

                if let Some(first) = values.first() {
                    let first_discriminant = std::mem::discriminant(first);
                    for value in &values {
                        if std::mem::discriminant(value) != first_discriminant {
                            return Err(ParserError::MixedTypesInArray);
                        }
                    }
                }

                Ok(Value::Tuple { direction, values })
            }
            Rule::tuple => {
                let mut values = Vec::new();
                for inner in pair.into_inner() {
                    values.push(self.visit_value(inner, direction)?);
                }
                Ok(Value::Tuple { direction, values })
            }
            _ => unreachable!("{:#?}", pair),
        }
    }

    pub fn visit_expression(&self, expression: Pairs<Rule>) -> Result<Expression, ParserError> {
        expression_parser()
            .map_primary(|primary| -> Result<Expression, ParserError> {
                match primary.as_rule() {
                    Rule::number => Ok(Expression::Number(primary.as_str().to_string())),
                    Rule::string => Ok(Expression::String(primary.as_str().to_string())),
                    Rule::relation => {
                        let mut pairs = primary.into_inner();
                        let identifier_str = pairs.next().unwrap().as_str();
                        let property = pairs.next().unwrap().as_str().to_string();

                        let identifier = self
                            .aliases
                            .get(identifier_str)
                            .cloned()
                            .unwrap_or_else(|| identifier_str.to_string());

                        Ok(Expression::Reference { identifier, property })
                    }
                    Rule::identifier => {
                        let identifier_str = primary.as_str();
                        if let Some(value) = self.scope.get(identifier_str) {
                            match value {
                                Value::Numeric { value, .. } => Ok(Expression::Number(value.clone())),
                                Value::Relation {
                                    identifier, property, ..
                                } => Ok(Expression::Reference {
                                    identifier: identifier.clone(),
                                    property: property.clone(),
                                }),
                                _ => Ok(Expression::Reference {
                                    identifier: identifier_str.to_string(),
                                    property: "output".to_string(),
                                }),
                            }
                        } else {
                            let identifier = self
                                .aliases
                                .get(identifier_str)
                                .cloned()
                                .unwrap_or_else(|| identifier_str.to_string());
                            Ok(Expression::Reference {
                                identifier,
                                property: "output".to_string(),
                            })
                        }
                    }
                    Rule::expression => self.visit_expression(primary.into_inner()),
                    rule => unreachable!("unexpected primary rule: {:?}", rule),
                }
            })
            .map_infix(|lhs, op, rhs| {
                let (lhs, rhs) = (lhs?, rhs?);
                let op = match op.as_rule() {
                    Rule::add => Operation::Add,
                    Rule::subtract => Operation::Subtract,
                    Rule::multiply => Operation::Multiply,
                    Rule::divide => Operation::Divide,
                    Rule::power => Operation::Power,
                    _ => unreachable!(),
                };
                Ok(Expression::BinaryOperation {
                    operation: op,
                    left: Box::new(lhs),
                    right: Box::new(rhs),
                })
            })
            .parse(expression)
    }

    fn collect_relations(
        source_identifier: Identifier,
        source_property: String,
        value: &Value,
        updates: &mut Vec<(Identifier, Property, Value)>,
    ) {
        match value {
            Value::Relation {
                identifier,
                direction,
                property,
            } => {
                if identifier == EVENT_PAYLOAD_IDENTIFIER {
                    return;
                }

                if identifier.starts_with(EVENT_ALIAS_IDENTIFIER_PREFIX) {
                    return;
                }

                updates.push((
                    identifier.clone(),
                    property.clone(),
                    Value::Relation {
                        identifier: source_identifier,
                        direction: direction.reverse(),
                        property: source_property,
                    },
                ));
            }
            Value::Tuple { values, .. } => {
                for value in values {
                    Self::collect_relations(source_identifier.clone(), source_property.clone(), value, updates);
                }
            }
            _ => {}
        }
    }

    /// Add bidirectional relations between linked nodes.
    pub fn link(&mut self) -> Result<(), ParserError> {
        let mut updates: Vec<(Identifier, Property, Value)> = Vec::new();

        for (_, node) in self.nodes.iter() {
            for (name, value) in &node.inputs {
                Self::collect_relations(node.identifier.clone(), name.clone(), value, &mut updates);
            }
        }

        for (target_id, target_prop, relation) in updates {
            match self.nodes.get_mut(&target_id) {
                Some(related) => {
                    if related.inputs.contains_key(&target_prop) {
                        continue;
                    }

                    related.inputs.insert(target_prop, relation);
                }
                None => {
                    if self.inputs.contains_key(&target_id) {
                        continue;
                    }

                    return Err(ParserError::ModuleNotDefined { identifier: target_id });
                }
            }
        }

        Ok(())
    }
}

pub const PIPELINE_RESULT_ID: &str = "__pipeline_result__";

#[derive(Debug)]
pub struct NodeParser<'a> {
    visitor: Visitor,
    inner: Option<Pair<'a, Rule>>,
}

impl<'a> NodeParser<'a> {
    pub fn new(source: &'a str) -> Result<Self, ParserError> {
        Ok(NodeParser {
            visitor: Visitor::default(),
            inner: Schema::parse(Rule::nodes, source)?.next(),
        })
    }

    pub fn with_inputs(mut self, inputs: HashMap<Identifier, SharedValue>) -> Self {
        self.visitor.external_inputs = inputs;
        self
    }

    pub fn parse(source: &'a str) -> Result<ParsedWorkflow, ParserError> {
        Self::new(source)?.evaluate()
    }

    pub fn evaluate(mut self) -> Result<ParsedWorkflow, ParserError> {
        if let Some(inner) = self.inner {
            for pair in inner.into_inner() {
                match pair.as_rule() {
                    Rule::node | Rule::anonymous_node => {
                        self.visitor.visit_node(pair)?;
                    }
                    Rule::sequence_group => {
                        self.visitor.visit_sequence_group(pair)?;
                    }
                    Rule::pipeline_result => {
                        self.visitor.visit_pipeline_result(pair)?;
                    }
                    Rule::for_loop => {
                        self.visitor.visit_for_loop(pair)?;
                    }
                    Rule::input_def => {
                        self.visitor.visit_input_def(pair)?;
                    }
                    Rule::EOI => continue,
                    _ => unreachable!(),
                };
            }
        }

        self.visitor.link()?;

        Ok(ParsedWorkflow {
            nodes: self.visitor.nodes,
            inputs: self.visitor.inputs,
            event_handlers: self.visitor.event_handlers,
            event_callback_nodes: self.visitor.event_callback_nodes,
            sequential_edges: self.visitor.sequential_edges,
        })
    }
}

fn convert_shared_value_to_parser_value(value: &SharedValue) -> Option<Value> {
    if TypeId::of::<String>() == value.as_ref().type_id() {
        if let Some(v) = value.downcast_ref::<String>() {
            return Some(Value::String {
                direction: Direction::Input,
                parts: vec![StringPart::Literal(v.clone())],
            });
        }
    }

    if TypeId::of::<&str>() == value.as_ref().type_id() {
        if let Some(value) = value.downcast_ref::<&str>() {
            return Some(Value::String {
                direction: Direction::Input,
                parts: vec![StringPart::Literal(value.to_string())],
            });
        }
    }

    macro_rules! check_numeric {
        ($($t:ty),*) => {
            $(
                if TypeId::of::<$t>() == value.as_ref().type_id() {
                    if let Some(v) = value.downcast_ref::<$t>() {
                        return Some(Value::Numeric {
                            direction: Direction::Input,
                            value: v.to_string(),
                        });
                    }
                }
            )*
        };
    }

    check_numeric!(u8, u16, u32, u64, u128, usize, i8, i16, i32, i64, i128, isize, f32, f64);

    if TypeId::of::<bool>() == value.as_ref().type_id() {
        if let Some(value) = value.downcast_ref::<bool>() {
            return Some(Value::Boolean {
                direction: Direction::Input,
                value: *value,
            });
        }
    }

    if TypeId::of::<Vec<String>>() == value.as_ref().type_id() {
        if let Some(value) = value.downcast_ref::<Vec<String>>() {
            return Some(Value::Tuple {
                direction: Direction::Input,
                values: value
                    .iter()
                    .map(|token| Value::String {
                        direction: Direction::Input,
                        parts: vec![StringPart::Literal(token.clone())],
                    })
                    .collect(),
            });
        }
    }

    if TypeId::of::<Vec<&str>>() == value.as_ref().type_id() {
        if let Some(value) = value.downcast_ref::<Vec<&str>>() {
            return Some(Value::Tuple {
                direction: Direction::Input,
                values: value
                    .iter()
                    .map(|token| Value::String {
                        direction: Direction::Input,
                        parts: vec![StringPart::Literal(token.to_string())],
                    })
                    .collect(),
            });
        }
    }

    None
}
