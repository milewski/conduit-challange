use crate::dsl::error::ParserError;
use bevy_ecs::component::Component;
use pest::Parser;
use pest::iterators::{Pair, Pairs};
use pest_derive::Parser;
use std::collections::{BTreeMap, HashMap};
use std::error::Error;
use std::ops::Index;
use pest::pratt_parser::PrattParser;
use uuid::Uuid;

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
        value: bool,
    },
    Relation {
        identifier: Identifier,
        direction: Direction,
        property: String,
    },
}

#[derive(Debug, PartialEq, Clone, Copy, Eq, Hash, Component)]
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

#[derive(Debug, Default)]
struct NewVisitor {
    nodes: BTreeMap<Identifier, NodeInstruct>,
}

type Identifier = String;
type Property = String;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct NodeInstruct {
    pub identifier: Identifier,
    pub module: String,
    pub inputs: HashMap<Property, Value>,
}

impl NodeInstruct {
    pub fn new(identifier: Option<&str>, module: &str) -> Self {
        NodeInstruct {
            identifier: identifier
                .map(|identifier| identifier.to_string())
                .unwrap_or_else(|| Uuid::new_v4().to_string()),
            module: module.to_string(),
            inputs: HashMap::default(),
        }
    }
}

impl NewVisitor {
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
                a => unreachable!("{:#?}", pair),
            };

            node.inputs.insert(property.as_str().to_string(), value);
        }

        Ok(())
    }
    
    pub fn visit_expression(&self, expression: Pairs<Rule>) -> Result<(String, String), ParserError> {
        // PrattParser::new().map_primary(expression)
    }
}

#[derive(Debug)]
struct NewParser<'a> {
    visitor: NewVisitor,
    inner: Option<Pair<'a, Rule>>,
}

impl<'a> NewParser<'a> {
    pub fn parse(source: &'a str) -> Result<BTreeMap<Identifier, NodeInstruct>, Box<dyn Error>> {
        let instance = NewParser {
            visitor: NewVisitor::default(),
            inner: Schema::parse(Rule::nodes, source)?.next(),
        };

        instance.evaluate()
    }

    pub fn evaluate(mut self) -> Result<BTreeMap<Identifier, NodeInstruct>, Box<dyn Error>> {
        if let Some(inner) = self.inner {
            for pair in inner.into_inner() {
                match pair.as_rule() {
                    Rule::node | Rule::anonymous_node => self.visitor.visit_node(pair)?,
                    Rule::EOI => continue,
                    _ => unreachable!(),
                };
            }
        }

        Ok(self.visitor.nodes)
    }
}

#[macro_export]
macro_rules! assert_parser_snapshot {
    ( $( $input:expr ),+ $(,)? ) => {
        $(
            insta::assert_debug_snapshot!(NewParser::parse($input));
        )+
    };
}

#[test]
fn test() {
    // assert_parser_snapshot!(
    //     "named module {}",
    //     "anonymous_module {}",
    //     "anonymous_module { a <- true }",
    //     "anonymous_module { a <- false }",
    //     "anonymous_module { a <- 123 }",
    //     "anonymous_module { a <- -123 }",
    //     "anonymous_module { a <- 123.123 }",
    //     "anonymous_module { a <- -123.123 }",
    //     r#"anonymous_module { a <- "string" }"#,
    //     r#"anonymous_module { a <- "string with space" }"#,
    // );
    //
    // assert_parser_snapshot!(
    //     "name_a module_a { property_a <- anonymous_module_b::custom {} }",
    //     "name_a module_a { property_a <- anonymous_module_b {} }",
    //     "name_a module_a { property_a <- name_b module_b::custom {} }",
    //     "name_a module_a { property_a <- name_b module_b {} }",
    // );

    // assert_parser_snapshot!(
    //     r#"
    //         name_a module_a { property_a <- 123 }
    //         name_b module_b { property_b <- name_a::property_a }
    //         name_c module_c { property_c <- name_b::property_b }
    //         name_d module_d { property_d -> name_a }
    //         name_e module_e { property_e -> name_a::custom_e }
    //         name_f module_f {
    //             property_f_1 -> name_g module_g { property_g -> name_a::custom_g }
    //             property_f_2 -> module_h { property_h -> name_a::custom_h }
    //         }
    //     "#
    // );

    assert_parser_snapshot!(
        "name module { property <- (123 + 123) }",
    );

    // insta::assert_debug_snapshot!(NewParser::parse("named module {}"));
    // insta::assert_debug_snapshot!(NewParser::parse("anonymous_module {}"));
    // insta::assert_debug_snapshot!(NewParser::parse("anonymous_module { a <- 123 }"));

    // todo!(
    //     "{:#?}",
    //     NewParser::parse(r#"anonymous_module { a <- "string" }")
    // );
}
