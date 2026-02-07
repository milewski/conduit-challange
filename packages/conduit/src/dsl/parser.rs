use crate::dsl::error::ParserError;
use pest::Parser;
use pest::iterators::{Pair, Pairs};
use pest::pratt_parser::{Assoc, Op, PrattParser};
use pest_derive::Parser;
use std::collections::BTreeMap;
use std::ops::Index;
use uuid::Uuid;

#[derive(Debug, PartialEq, Clone, Eq, Hash)]
pub enum Expression {
    Number(String),
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
pub enum Value {
    String {
        direction: Direction,
        value: String,
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
}

impl Value {
    pub fn direction(&self) -> Direction {
        match self {
            Value::String { direction, .. }
            | Value::Numeric { direction, .. }
            | Value::Boolean { direction, .. }
            | Value::Expression { direction, .. }
            | Value::Relation { direction, .. } => *direction,
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

#[derive(Debug, PartialEq, Clone, Eq, Hash)]
pub struct ParsedWorkflow {
    pub nodes: BTreeMap<Identifier, NodeInstruct>,
    pub inputs: BTreeMap<Identifier, Option<Value>>,
}

#[derive(Debug, Default)]
struct Visitor {
    nodes: BTreeMap<Identifier, NodeInstruct>,
    inputs: BTreeMap<Identifier, Option<Value>>,
}

impl Visitor {
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

        let mut module = module.into_inner();
        let (module_identifier, module_property) = (module.next().unwrap_or_else(|| unreachable!()), module.next());

        let mut node = NodeInstruct::new(identifier, module_identifier.as_str());

        if self.nodes.contains_key(&node.identifier) {
            return Err(ParserError::DuplicatedNode {
                identifier: node.identifier,
            });
        }

        match body_or_shorthand.as_rule() {
            Rule::body => self.visit_body(&mut node, body_or_shorthand)?,
            Rule::shorthand => self.visit_shorthand(&mut node, body_or_shorthand)?,
            _ => unreachable!(),
        }

        let identifier = node.identifier.clone();

        self.nodes.insert(identifier.clone(), node);

        Ok((self.nodes.index(&identifier), module_property))
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
            assert_eq!(pair.as_rule(), Rule::parameter);

            let mut pairs = pair.into_inner();

            let (property, direction, value) = (
                pairs.next().unwrap_or_else(|| unreachable!()),
                pairs.next().unwrap_or_else(|| unreachable!()),
                pairs.next().unwrap_or_else(|| unreachable!()),
            );

            assert_eq!(property.as_rule(), Rule::property);
            assert_eq!(direction.as_rule(), Rule::direction);
            assert_eq!(value.as_rule(), Rule::value);

            let inner = value.into_inner().next().unwrap_or_else(|| unreachable!());
            let direction: Direction = direction.into();

            let value = self.visit_value(inner, direction)?;

            node.inputs.insert(property.as_str().to_string(), value);
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
        } else {
            None
        };

        if self.inputs.contains_key(&name) {
            return Err(ParserError::DuplicatedNode { identifier: name });
        }
        self.inputs.insert(name, value);
        Ok(())
    }

    pub fn visit_pipeline_result(&mut self, pair: Pair<Rule>) -> Result<(), ParserError> {
        assert_eq!(pair.as_rule(), Rule::pipeline_result);

        let value_pair = pair.into_inner().next().unwrap_or_else(|| unreachable!());
        let inner = value_pair.into_inner().next().unwrap_or_else(|| unreachable!());
        let direction = Direction::Input;
        let value = self.visit_value(inner, direction)?;

        let mut node = NodeInstruct::new(Some(PIPELINE_RESULT_ID), "_");
        node.inputs.insert("input".to_string(), value);
        self.nodes.insert(PIPELINE_RESULT_ID.to_string(), node);

        Ok(())
    }

    fn visit_value(&mut self, pair: Pair<Rule>, direction: Direction) -> Result<Value, ParserError> {
        match pair.as_rule() {
            Rule::expression => Ok(Value::Expression {
                direction,
                value: self.visit_expression(pair.into_inner())?,
            }),
            Rule::number => Ok(Value::Numeric {
                direction,
                value: pair.as_str().to_string(),
            }),
            Rule::string => Ok(Value::String {
                direction,
                value: pair.as_str().trim_matches('"').to_string(),
            }),
            Rule::boolean => Ok(Value::Boolean {
                direction,
                value: match pair.as_str() {
                    "true" => true,
                    "false" => false,
                    _ => unreachable!(),
                },
            }),
            Rule::identifier => Ok(Value::Relation {
                direction,
                identifier: pair.as_str().to_string(),
                property: direction.reverse().as_str().to_string(),
            }),
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

                let (identifier, related_property) = (
                    pairs.next().unwrap_or_else(|| unreachable!()),
                    pairs.next().unwrap_or_else(|| unreachable!()),
                );

                assert_eq!(identifier.as_rule(), Rule::identifier);
                assert_eq!(related_property.as_rule(), Rule::property);

                Ok(Value::Relation {
                    direction,
                    identifier: identifier.as_str().to_string(),
                    property: related_property.as_str().to_string(),
                })
            }
            _ => unreachable!("{:#?}", pair),
        }
    }

    pub fn visit_expression(&self, expression: Pairs<Rule>) -> Result<Expression, ParserError> {
        let pratt = PrattParser::new()
            .op(Op::infix(Rule::add, Assoc::Left) | Op::infix(Rule::subtract, Assoc::Left))
            .op(Op::infix(Rule::multiply, Assoc::Left) | Op::infix(Rule::divide, Assoc::Left))
            .op(Op::infix(Rule::power, Assoc::Right));

        pratt
            .map_primary(|primary| -> Result<Expression, ParserError> {
                match primary.as_rule() {
                    Rule::number => Ok(Expression::Number(primary.as_str().to_string())),
                    Rule::relation => {
                        let mut pairs = primary.into_inner();
                        let identifier = pairs.next().unwrap().as_str().to_string();
                        let property = pairs.next().unwrap().as_str().to_string();
                        Ok(Expression::Reference { identifier, property })
                    }
                    Rule::identifier => {
                        let identifier = primary.as_str().to_string();
                        Ok(Expression::Reference {
                            identifier,
                            property: "output".to_string(),
                        })
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

    /// Add bidirectional relations between linked nodes.
    pub fn link(&mut self) -> Result<(), ParserError> {
        let mut updates: Vec<(Identifier, Property, Value)> = Vec::new();

        for (_, node) in self.nodes.iter() {
            for (name, value) in &node.inputs {
                if let Value::Relation {
                    identifier,
                    direction,
                    property,
                } = value
                {
                    updates.push((
                        identifier.clone(),
                        property.clone(),
                        Value::Relation {
                            identifier: node.identifier.clone(),
                            direction: direction.reverse(),
                            property: name.clone(),
                        },
                    ));
                }
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
    pub fn parse(source: &'a str) -> Result<ParsedWorkflow, ParserError> {
        let instance = NodeParser {
            visitor: Visitor::default(),
            inner: Schema::parse(Rule::nodes, source)?.next(),
        };

        instance.evaluate()
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
}

#[cfg(test)]
mod input_tests {
    use super::*;

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
