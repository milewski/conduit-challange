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
pub struct CallbackAssignment {
    pub identifier: Identifier,
    pub property: Property,
    pub value: Value,
}

#[derive(Debug, PartialEq, Clone, Eq, Hash)]
pub enum EventCallback {
    Value(Value),
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
}

#[derive(Debug, Default)]
struct Visitor {
    nodes: BTreeMap<Identifier, NodeInstruct>,
    inputs: BTreeMap<Identifier, Option<Value>>,
    event_handlers: BTreeMap<Identifier, BTreeMap<String, Vec<EventCallback>>>,
    event_callback_nodes: HashSet<Identifier>,
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
            value: callback_value,
        })
    }

    fn parse_event_callback(&mut self, event_callback: Pair<Rule>) -> Result<EventCallback, ParserError> {
        assert_eq!(event_callback.as_rule(), Rule::event_callback);
        let callback_inner = event_callback.into_inner().next().unwrap_or_else(|| unreachable!());

        match callback_inner.as_rule() {
            Rule::value => {
                let callback_value_pair = callback_inner.into_inner().next().unwrap_or_else(|| unreachable!());
                let callback_value_rule = callback_value_pair.as_rule();
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
                        self.event_callback_nodes.insert(identifier.clone());
                        if *direction == Direction::Input && property == "output" {
                            *property = "input".to_string();
                        }
                    }
                }

                Ok(EventCallback::Value(callback_value))
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
                            let mut node_value = self.visit_value(statement, Direction::Input)?;
                            if let Value::Relation {
                                identifier,
                                direction,
                                property,
                            } = &mut node_value
                            {
                                if *direction == Direction::Input && property == "output" {
                                    *property = "input".to_string();
                                }
                                self.event_callback_nodes.insert(identifier.clone());
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
        let callback_pair = pairs.next().unwrap_or_else(|| unreachable!());
        let parsed_callback = self.parse_event_callback(callback_pair)?;

        let event_names: Vec<String> = match event_names_pair.as_rule() {
            Rule::event_names => event_names_pair
                .into_inner()
                .map(|event_name_pair| event_name_pair.as_str().to_string())
                .collect(),
            _ => unreachable!(),
        };

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

            // Map the assignment property (which was parsed as module property)
            // e.g. store::counter -> module_property is "counter"
            let target_prop = module_property.as_ref().unwrap().as_str().to_string();

            // We'll process body/shorthand and insert values into this new node
            // But wait, the shorthand/body logic inserts into `node.inputs`.
            // If the shorthand is just `<- val`, it puts `val` into input named "input" (or from direction).
            // We want it in `target_prop`.

            // Special handling for assignment body:
            // If shorthand: `<- value` -> insert into `target_prop`.
            // If body: `parameter` -> insert into `parameter`.
            // BUT strict assignment syntax `store::counter <- val` corresponds to shorthand.

            new_node.inputs.insert(
                target_prop.clone(),
                Value::Numeric {
                    direction: Direction::Input,
                    value: "0".to_string(),
                },
            ); // Placeholder

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
                    let _ = pairs.next();
                    let pair = pairs.next().unwrap().into_inner().next().unwrap();
                    let value = self.visit_value(pair, Direction::Input)?;
                    node.inputs.insert(target_prop, value);
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

                    let (property_name, direction_pair, value_pair) = if first.as_rule() == Rule::property {
                        let direction = pairs.next().unwrap_or_else(|| unreachable!());
                        let value = pairs.next().unwrap_or_else(|| unreachable!());
                        (first.as_str().to_string(), direction, value)
                    } else if first.as_rule() == Rule::direction {
                        let direction = first;
                        let value = pairs.next().unwrap_or_else(|| unreachable!());
                        let direction_string = direction.as_str();
                        let property_name = match direction_string {
                            "<-" => "input".to_string(),
                            "->" => "output".to_string(),
                            _ => unreachable!(),
                        };
                        (property_name, direction, value)
                    } else {
                        unreachable!("Unexpected rule in parameter: {:?}", first.as_rule());
                    };

                    assert_eq!(direction_pair.as_rule(), Rule::direction);
                    assert_eq!(value_pair.as_rule(), Rule::value);

                    let inner = value_pair.into_inner().next().unwrap_or_else(|| unreachable!());
                    let direction: Direction = direction_pair.into();
                    let value = self.visit_value(inner, direction)?;
                    node.inputs.insert(property_name, value);
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
        let mut pairs = pair.into_inner();

        let identifier = pairs.next().unwrap_or_else(|| unreachable!());
        assert_eq!(identifier.as_rule(), Rule::identifier);
        let name = identifier.as_str().to_string();

        let value = if let Some(val_pair) = pairs.next() {
            // Check if there is a value provided
            let inner = val_pair.into_inner().next().unwrap_or_else(|| unreachable!());
            Some(self.visit_value(inner, Direction::Input)?)
        } else if let Some(external_val) = self.external_inputs.get(&name) {
            convert_shared_value_to_parser_value(external_val)
        } else {
            None
        };

        if self.inputs.contains_key(&name) {
            return Err(ParserError::DuplicatedNode { identifier: name });
        }
        self.inputs.insert(name, value);
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
            .map_infix(|left, op, right| {
                let left = left?;
                let right = right?;
                match op.as_rule() {
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
        // Check scope (loop variables)
        if let Some(val) = self.scope.get(name) {
            return self.value_to_int(val);
        }
        // Check inputs
        if let Some(Some(val)) = self.inputs.get(name) {
            return self.value_to_int(val);
        }

        Err(ParserError::ConstantNotFound(name.to_string()))
    }

    fn evaluate_relation_constant(&self, pair: Pair<Rule>) -> Result<i32, ParserError> {
        let mut pairs = pair.into_inner();
        let identifier = pairs.next().unwrap().as_str();
        let property = pairs.next().unwrap().as_str();

        let resolved_id = self.aliases.get(identifier).map(|s| s.as_str()).unwrap_or(identifier);

        if let Some(node) = self.nodes.get(resolved_id) {
            if let Some(val) = node.inputs.get(property) {
                return self.value_to_int(val);
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

        let value_pair = pair.into_inner().next().unwrap_or_else(|| unreachable!());
        let inner = value_pair.into_inner().next().unwrap_or_else(|| unreachable!());
        let direction = Direction::Input;
        let value = self.visit_value(inner, direction)?;

        // If the result value is a Relation, it might point to an alias.
        // visit_value should have already resolved it.
        // But double check if we need special handling for pipeline result.

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

        Ok(())
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
                                _ => unreachable!("Only numeric values are supported in expressions"),
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
        })
    }
}

#[macro_export]
macro_rules! assert_parser_snapshot {
    ( $( $input:expr ),+ $(,)? ) => {
        $(
            insta::with_settings!({
                filters => vec![(r"[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}", "[UUID]")]
            }, {
                let nodes = NodeParser::parse($input).map(|res| res.nodes);
                insta::assert_debug_snapshot!(nodes);
            });
        )+
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_nodes() {
        assert_parser_snapshot!(
            "named module {}",
            "anonymous_module {}",
            "anonymous_module { a <- true }",
            "anonymous_module { a <- false }",
            "anonymous_module { a <- 123 }",
            "anonymous_module { a <- -123 }",
            "anonymous_module { a <- 123.123 }",
            "anonymous_module { a <- -123.123 }",
            r#"anonymous_module { a <- "string" }"#,
            r#"anonymous_module { a <- "string with space" }"#,
        );
    }

    #[test]
    fn test_inline_nodes() {
        assert_parser_snapshot!(
            "name_a module_a { property_a <- anonymous_module_b::custom {} }",
            "name_a module_a { property_a <- anonymous_module_b {} }",
            "name_a module_a { property_a <- name_b module_b::custom {} }",
            "name_a module_a { property_a <- name_b module_b {} }",
        );
    }

    #[test]
    fn test_relations_and_chaining() {
        assert_parser_snapshot!(
            r#"
                name_a module_a { property_a <- 123 }
                name_b module_b { property_b <- name_a::property_a }
                name_c module_c { property_c <- name_b::property_b }
                name_d module_d { property_d -> name_a }
                name_e module_e { property_e -> name_a::custom_e }
                name_f module_f {
                    property_f_1 -> name_g module_g { property_g -> name_a::custom_g }
                    property_f_2 -> name_h module_h { property_h -> name_a::custom_h }
                }
            "#
        );
    }

    #[test]
    fn test_expression_basic() {
        assert_parser_snapshot!("name module { property <- (123 + 123) }",);
    }

    #[test]
    fn test_expression_arithmetic() {
        assert_parser_snapshot!(
            "name module { property <- (10 - 3) }",
            "name module { property <- (6 * 7) }",
            "name module { property <- (100 / 4) }",
            "name module { property <- (2 ^ 10) }",
        );
    }

    #[test]
    fn test_expression_precedence() {
        assert_parser_snapshot!(
            "name module { property <- (2 + 3 * 4) }",
            "name module { property <- ((2 + 3) * 4) }",
            "name module { property <- (10 - 2 * 3) }",
            "name module { property <- (10 / 2 + 3) }",
        );
    }

    #[test]
    fn test_expression_nested() {
        assert_parser_snapshot!(
            "name module { property <- ((1 + 2) * (3 + 4)) }",
            "name module { property <- (((10))) }",
        );
    }

    #[test]
    fn test_duplicated_node_error() {
        assert_parser_snapshot!(
            r#"
                name module_a {}
                name module_b {}
            "#,
        );
    }

    #[test]
    fn test_expression_with_node_ref() {
        assert_parser_snapshot!(
            r#"
                config constants { multiplier <- 4 }
                name module { width <- (32 * config::multiplier) }
            "#,
            r#"
                a module_a { x <- 10 }
                b module_b { y <- (a::x + 5) }
            "#,
            r#"
                a module_a { x <- 10 }
                b module_b { y <- 20 }
                c module_c { z <- (a::x * b::y + 1) }
            "#,
        );
    }

    #[test]
    fn test_shorthand_basic() {
        assert_parser_snapshot!(
            // anonymous node with shorthand input
            r#"module_a <- "hello""#,
            // named node with shorthand input
            r#"name module_a <- "hello""#,
            // shorthand with number
            r#"name module_a <- 42"#,
            // shorthand with boolean
            r#"name module_a <- true"#,
            // shorthand with identifier (node ref)
            r#"
                name_a module_a <- 123
                name_b module_b <- name_a
            "#,
            // shorthand with output direction
            r#"
                name_a module_a <- 123
                name_b module_b -> name_a
            "#,
        );
    }

    #[test]
    fn test_shorthand_chaining() {
        assert_parser_snapshot!(
            // chaining: named nodes
            r#"name_a module_a <- name_b module_b <- "hello""#,
            // chaining: anonymous inner node
            r#"name_a module_a <- module_b <- "hello""#,
            // triple chain
            r#"name_a module_a <- name_b module_b <- name_c module_c <- 42"#,
        );
    }

    #[test]
    fn test_shorthand_mixed_with_body() {
        assert_parser_snapshot!(
            // outer uses body, inner uses shorthand
            r#"
                name_a module_a {
                    input <- name_b module_b <- "hello"
                    extra <- 123
                }
            "#,
            // shorthand node used as value alongside body nodes
            r#"
                name_a module_a {
                    input <- name_b module_b <- "hello"
                    output -> name_c module_c <- 42
                }
            "#,
        );
    }
    #[test]
    fn test_string_interpolation_parsing_simple() {
        assert_parser_snapshot!(r#"node m { s <- "Hello { config::name }!" }"#,);
    }

    #[test]
    fn test_implicit_property_assignment() {
        assert_parser_snapshot!(
            r#"
                name module {
                    <- "implicit input"
                    -> "implicit output"
                    other <- 123
                }
            "#,
        );
    }

    #[test]
    fn test_pipeline_inputs() {
        let input = r#"
            -> width <- 100
            -> height <- 200
            node module {
                w <- width
                h <- height
            }
        "#;
        let workflow = NodeParser::parse(input).expect("Failed to parse");
        assert!(workflow.inputs.contains_key("width"));
        assert!(workflow.inputs.contains_key("height"));

        let width_val = workflow.inputs.get("width").unwrap();
        // 100 is parsed as Numeric
        if let Some(Value::Numeric { value, .. }) = width_val {
            assert_eq!(value, "100");
        } else {
            panic!("Expected Numeric value for width");
        }
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
