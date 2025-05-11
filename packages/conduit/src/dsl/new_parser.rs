use crate::dsl::error::ParserError;
use bevy_ecs::component::Component;
use pest::iterators::Pair;
use pest::Parser;
use pest_derive::Parser;
use std::collections::HashMap;
use std::error::Error;
use std::ops::Index;
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
    nodes: HashMap<Identifier, NodeInstruct>,
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

            let value = match pair.as_rule() {
                Rule::number => Value::Numeric {
                    direction: direction.into(),
                    value: pair.as_str().to_string(),
                },
                Rule::string => Value::String {
                    direction: direction.into(),
                    value: pair.as_str().trim_matches('"').to_string(),
                },
                Rule::boolean => Value::Boolean {
                    direction: direction.into(),
                    value: match pair.as_str() {
                        "true" => true,
                        "false" => false,
                        _ => unreachable!(),
                    },
                },
                Rule::node | Rule::anonymous_node => {
                    let direction: Direction = direction.into();
                    let (node, property) = self.visit_node(pair)?;

                    Value::Relation {
                        direction,
                        property: property
                            .map(|property| property.as_str().to_string())
                            .unwrap_or_else(|| direction.reverse().as_str().to_string()),
                        identifier: node.identifier.clone(),
                    }
                }
                // Rule::relation => {
                //     let mut pairs = pair.into_inner();
                //
                //     todo!("{:#?}", pairs);
                //
                //     // if let (Some(identifier), Some(related_property)) = (pairs.next(), pairs.next()) {
                //     //     Value::Relation {
                //     //         direction: direction.into(),
                //     //         identifier: Identifier(identifier.as_str().to_string()),
                //     //         property: related_property.as_str().to_string(),
                //     //     }
                //     // } else {
                //     //     unreachable!()
                //     // }
                // }
                _ => unreachable!(),
            };

            node.inputs.insert(property.as_str().to_string(), value);
        }

        Ok(())
    }
}

#[derive(Debug)]
struct NewParser<'a> {
    visitor: NewVisitor,
    inner: Option<Pair<'a, Rule>>,
}

impl<'a> NewParser<'a> {
    pub fn parse(source: &'a str) -> Result<HashMap<Identifier, NodeInstruct>, Box<dyn Error>> {
        let instance = NewParser {
            visitor: NewVisitor::default(),
            inner: Schema::parse(Rule::nodes, source)?.next(),
        };

        instance.evaluate()
    }

    pub fn evaluate(mut self) -> Result<HashMap<Identifier, NodeInstruct>, Box<dyn Error>> {
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

    assert_parser_snapshot!(
        "name_a module_a { property_a <- anonymous_module_b::custom {} }",
        "name_a module_a { property_a <- anonymous_module_b {} }",
        "name_a module_a { property_a <- name_b module_b::custom {} }",
        "name_a module_a { property_a <- name_b module_b {} }",
    );

    // insta::assert_debug_snapshot!(NewParser::parse("named module {}"));
    // insta::assert_debug_snapshot!(NewParser::parse("anonymous_module {}"));
    // insta::assert_debug_snapshot!(NewParser::parse("anonymous_module { a <- 123 }"));

    // todo!(
    //     "{:#?}",
    //     NewParser::parse(r#"anonymous_module { a <- "string" }")
    // );
}
