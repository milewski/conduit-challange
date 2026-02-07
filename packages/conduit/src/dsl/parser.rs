use crate::dsl::error::ParserError;
use pest::Parser;
use pest::iterators::{Pair, Pairs};
use pest::pratt_parser::{Assoc, Op, PrattParser};
use pest_derive::Parser;
use std::collections::{BTreeMap, HashMap};
use std::ops::Index;
use uuid::Uuid;

#[derive(Debug, PartialEq, Clone, Eq, Hash)]
pub enum Expr {
    Number(String),
    Ref { identifier: String, property: String },
    BinOp { op: ExprOp, left: Box<Expr>, right: Box<Expr> },
}

#[derive(Debug, PartialEq, Clone, Copy, Eq, Hash)]
pub enum ExprOp {
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
        value: Expr,
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

#[derive(Debug, Clone, Eq, PartialEq)]
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

#[derive(Debug, Default)]
struct Visitor {
    nodes: BTreeMap<Identifier, NodeInstruct>,
}

impl Visitor {
    pub fn visit_node<'a>(&'a mut self, node: Pair<'a, Rule>) -> Result<(&'a NodeInstruct, Option<Pair<'a, Rule>>), ParserError> {
        let kind = node.as_rule();
        let mut pairs = node.into_inner();

        let (identifier, module, body) = match kind {
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

        self.visit_body(&mut node, body)?;

        let identifier = node.identifier.clone();

        self.nodes.insert(identifier.clone(), node);

        Ok((self.nodes.index(&identifier), module_property))
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

            let pair = value.into_inner().next().unwrap_or_else(|| unreachable!());
            let direction = direction.into();

            let value = match pair.as_rule() {
                Rule::expression => Value::Expression {
                    direction,
                    value: self.visit_expression(pair.into_inner())?,
                },
                Rule::number => Value::Numeric {
                    direction,
                    value: pair.as_str().to_string(),
                },
                Rule::string => Value::String {
                    direction,
                    value: pair.as_str().trim_matches('"').to_string(),
                },
                Rule::boolean => Value::Boolean {
                    direction,
                    value: match pair.as_str() {
                        "true" => true,
                        "false" => false,
                        _ => unreachable!(),
                    },
                },
                Rule::identifier => Value::Relation {
                    direction,
                    identifier: pair.as_str().to_string(),
                    property: direction.reverse().as_str().to_string(),
                },
                Rule::node | Rule::anonymous_node => {
                    let (node, property) = self.visit_node(pair)?;

                    Value::Relation {
                        direction,
                        property: property
                            .map(|property| property.as_str().to_string())
                            .unwrap_or_else(|| direction.reverse().as_str().to_string()),
                        identifier: node.identifier.clone(),
                    }
                }
                Rule::relation => {
                    let mut pairs = pair.into_inner();

                    let (identifier, related_property) = (
                        pairs.next().unwrap_or_else(|| unreachable!()),
                        pairs.next().unwrap_or_else(|| unreachable!()),
                    );

                    assert_eq!(identifier.as_rule(), Rule::identifier);
                    assert_eq!(related_property.as_rule(), Rule::property);

                    Value::Relation {
                        direction,
                        identifier: identifier.as_str().to_string(),
                        property: related_property.as_str().to_string(),
                    }
                }
                _ => unreachable!("{:#?}", pair),
            };

            node.inputs.insert(property.as_str().to_string(), value);
        }

        Ok(())
    }

    pub fn visit_expression(&self, expression: Pairs<Rule>) -> Result<Expr, ParserError> {
        let pratt = PrattParser::new()
            .op(Op::infix(Rule::add, Assoc::Left) | Op::infix(Rule::subtract, Assoc::Left))
            .op(Op::infix(Rule::multiply, Assoc::Left) | Op::infix(Rule::divide, Assoc::Left))
            .op(Op::infix(Rule::power, Assoc::Right));

        pratt
            .map_primary(|primary| -> Result<Expr, ParserError> {
                match primary.as_rule() {
                    Rule::number => Ok(Expr::Number(primary.as_str().to_string())),
                    Rule::relation => {
                        let mut pairs = primary.into_inner();
                        let identifier = pairs.next().unwrap().as_str().to_string();
                        let property = pairs.next().unwrap().as_str().to_string();
                        Ok(Expr::Ref { identifier, property })
                    }
                    Rule::expression => self.visit_expression(primary.into_inner()),
                    rule => unreachable!("unexpected primary rule: {:?}", rule),
                }
            })
            .map_infix(|lhs, op, rhs| {
                let (lhs, rhs) = (lhs?, rhs?);
                let op = match op.as_rule() {
                    Rule::add => ExprOp::Add,
                    Rule::subtract => ExprOp::Subtract,
                    Rule::multiply => ExprOp::Multiply,
                    Rule::divide => ExprOp::Divide,
                    Rule::power => ExprOp::Power,
                    _ => unreachable!(),
                };
                Ok(Expr::BinOp {
                    op,
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
                    return Err(ParserError::ModuleNotDefined { identifier: target_id });
                }
            }
        }

        Ok(())
    }
}

#[derive(Debug)]
pub struct NodeParser<'a> {
    visitor: Visitor,
    inner: Option<Pair<'a, Rule>>,
}

impl<'a> NodeParser<'a> {
    pub fn parse(source: &'a str) -> Result<BTreeMap<Identifier, NodeInstruct>, ParserError> {
        let instance = NodeParser {
            visitor: Visitor::default(),
            inner: Schema::parse(Rule::nodes, source)?.next(),
        };

        instance.evaluate()
    }

    pub fn evaluate(mut self) -> Result<BTreeMap<Identifier, NodeInstruct>, ParserError> {
        if let Some(inner) = self.inner {
            for pair in inner.into_inner() {
                match pair.as_rule() {
                    Rule::node | Rule::anonymous_node => {
                        self.visitor.visit_node(pair)?;
                    }
                    Rule::EOI => continue,
                    _ => unreachable!(),
                };
            }
        }

        self.visitor.link()?;
        Ok(self.visitor.nodes)
    }
}

#[macro_export]
macro_rules! assert_parser_snapshot {
    ( $( $input:expr ),+ $(,)? ) => {
        $(
            insta::with_settings!({
                filters => vec![(r"[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}", "[UUID]")]
            }, {
                insta::assert_debug_snapshot!(NodeParser::parse($input));
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
}
