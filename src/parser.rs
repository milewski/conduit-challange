use bevy_ecs::prelude::Component;
use pest::iterators::Pair;
use pest::Parser;
use pest_derive::Parser;
use std::any::Any;
use std::collections::BTreeMap;
use std::fmt::{Debug, Display, Formatter};
use std::ops::Deref;
use uuid::Uuid;

#[derive(thiserror::Error, Debug)]
pub enum ParserError {
    #[error("data store disconnected")]
    ModuleNotDefined { identifier: String },

    #[error("data store disconnected")]
    PropertyConflict { identifier: String, property: String },

    #[error("data store disconnected")]
    DuplicatedNode { identifier: String },

    #[error("data store disconnected")]
    ParserError(#[from] pest::error::Error<Rule>),
}

#[derive(Parser, Debug)]
#[grammar = "schema.pest"]
pub struct Parser {
    inner: Pair<'static, Rule>,
}

#[derive(Debug, Clone, Eq, Hash, PartialEq, Ord, PartialOrd, Component)]
pub struct Identifier(pub String);

impl Deref for Identifier {
    type Target = String;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Display for Identifier {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<&str> for Identifier {
    fn from(value: &str) -> Self {
        Identifier(value.to_string())
    }
}

#[derive(Debug, PartialEq, Clone, Component)]
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

impl Value {
    pub fn direction(&self) -> Direction {
        match self {
            Value::String { direction, .. } => direction.clone(),
            Value::Numeric { direction, .. } => direction.clone(),
            Value::Boolean { direction, .. } => direction.clone(),
            Value::Relation { direction, .. } => direction.clone(),
        }
    }
}

#[derive(Debug, Clone, Component)]
pub struct NodeInstruct {
    pub identifier: Identifier,
    pub module: String,
    pub inputs: BTreeMap<String, Value>,
}

impl NodeInstruct {
    fn new(module: &str, identifier: Option<&str>) -> Self {
        NodeInstruct {
            identifier: identifier
                .map(|identifier| Identifier(identifier.to_string()))
                .unwrap_or_else(|| Identifier(Uuid::new_v4().to_string())),
            module: module.to_string(),
            inputs: BTreeMap::default(),
        }
    }
}

#[derive(Debug, Default)]
struct Visitor {
    nodes: BTreeMap<Identifier, NodeInstruct>,
}

#[derive(Debug, PartialEq, Clone, Copy, Component)]
pub enum Direction {
    Input,
    Output,
}

impl From<&Pair<'_, Rule>> for Direction {
    fn from(value: &Pair<'_, Rule>) -> Self {
        match value.as_str() {
            "<-" => Direction::Input,
            "->" => Direction::Output,
            _ => unreachable!(),
        }
    }
}

impl Direction {
    fn reverse(self) -> Self {
        match self {
            Direction::Input => Direction::Output,
            Direction::Output => Direction::Input,
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Direction::Input => "input",
            Direction::Output => "output",
        }
    }

    fn as_symbol(self) -> &'static str {
        match self {
            Direction::Input => "<-",
            Direction::Output => "->",
        }
    }
}

impl Visitor {
    pub fn link(&mut self) -> Result<(), ParserError> {
        let mut updates = Vec::new();

        for (_, node) in self.nodes.iter_mut() {
            for (name, value) in &node.inputs.clone() {
                if let Value::Relation {
                    identifier,
                    direction,
                    property,
                } = value
                {
                    updates.push((
                        identifier.clone(),
                        property.clone(),
                        direction.clone(),
                        Value::Relation {
                            identifier: node.identifier.clone(),
                            direction: direction.reverse(),
                            property: name.clone(),
                        },
                    ));
                }
            }
        }

        for (identifier, property, direction, relation) in updates {
            match self.nodes.get_mut(&identifier) {
                Some(related) => {
                    if related.inputs.contains_key(&property) {
                        continue;
                    }

                    if let Some(related_value) = related.inputs.get(&property) {
                        if related_value.direction() != direction {
                            // continue;
                        }
                    }

                    related.inputs.insert(property, relation);
                }
                None => Err(ParserError::ModuleNotDefined {
                    identifier: identifier.0.to_string(),
                })?,
            }
        }

        Ok(())
    }
}

impl Visitor {
    pub fn visit_node(&mut self, node: Pair<Rule>) -> Result<&NodeInstruct, ParserError> {
        let mut pairs = node.into_inner();
        let first = pairs.next().unwrap_or_else(|| unreachable!()); // Ensure there's at least one element

        let (mut node, body) = match first.as_rule() {
            Rule::identifier => {
                let module = pairs.next().unwrap_or_else(|| unreachable!());
                let body = pairs.next().unwrap_or_else(|| unreachable!());
                (NodeInstruct::new(module.as_str(), Some(first.as_str())), body)
            }
            Rule::module => {
                let body = pairs.next().unwrap_or_else(|| unreachable!());
                (NodeInstruct::new(first.as_str(), None), body)
            }
            _ => unreachable!(),
        };

        self.visit_body(&mut node, body)?;

        let identifier = node.identifier.clone();

        if self.nodes.contains_key(&identifier) {
            Err(ParserError::DuplicatedNode {
                identifier: identifier.0.to_string(),
            })?
        }

        self.nodes.insert(identifier.clone(), node);
        Ok(self.nodes.get(&identifier).unwrap())
    }

    pub fn visit_body(&mut self, node: &mut NodeInstruct, body: Pair<Rule>) -> Result<(), ParserError> {
        for pair in body.into_inner() {
            assert_eq!(pair.as_rule(), Rule::parameter);

            let mut pairs = pair.into_inner();

            if let (Some(property), Some(direction), Some(value)) = (pairs.next(), &pairs.next(), pairs.next()) {
                assert_eq!(property.as_rule(), Rule::property);
                assert_eq!(direction.as_rule(), Rule::direction);

                for pair in value.into_inner() {
                    let value = match pair.as_rule() {
                        Rule::string => match pair.into_inner().next() {
                            None => unreachable!(),
                            Some(inner_string) => Value::String {
                                direction: direction.into(),
                                value: inner_string.as_str().to_string(),
                            }
                        }
                        Rule::number => Value::Numeric {
                            direction: direction.into(),
                            value: pair.as_str().to_string(),
                        },
                        Rule::node | Rule::anonymous_node => {
                            let direction: Direction = direction.into();

                            Value::Relation {
                                direction,
                                property: direction.reverse().as_str().to_string(),
                                identifier: self.visit_node(pair)?.identifier.clone(),
                            }
                        }
                        Rule::boolean => Value::Boolean {
                            direction: direction.into(),
                            value: match pair.as_str() {
                                "true" => true,
                                "false" => false,
                                _ => unreachable!(),
                            },
                        },
                        Rule::identifier => {
                            let direction = direction.into();
                            Value::Relation {
                                direction,
                                identifier: Identifier(pair.as_str().to_string()),
                                property: direction.reverse().as_str().to_string(),
                            }
                        }
                        Rule::relation => {
                            let mut pairs = pair.into_inner();

                            if let (Some(identifier), Some(related_property)) = (pairs.next(), pairs.next()) {
                                Value::Relation {
                                    direction: direction.into(),
                                    identifier: Identifier(identifier.as_str().to_string()),
                                    property: related_property.as_str().to_string(),
                                }
                            } else {
                                unreachable!()
                            }
                        }
                        _ => unreachable!(),
                    };

                    node.inputs.insert(property.as_str().to_string(), value);
                }
            }
        }

        Ok(())
    }
}

impl Parser {
    pub fn new(input: &str) -> Result<Self, ParserError> {
        // Convert the input to a 'static lifetime - this is safe because we're parsing it immediately
        // and not storing references to the original string
        let input_static: &'static str = Box::leak(input.to_string().into_boxed_str());

        Ok(Parser {
            inner: Parser::parse(Rule::nodes, input_static)?.next().expect("invalid input..."),
        })
    }

    pub fn evaluate(self) -> Result<BTreeMap<Identifier, NodeInstruct>, ParserError> {
        let mut visitor = Visitor::default();

        for pair in self.inner.into_inner() {
            match pair.as_rule() {
                Rule::node | Rule::anonymous_node => visitor.visit_node(pair)?,
                Rule::EOI => continue,
                _ => unreachable!(),
            };
        }

        visitor.link()?;

        Ok(visitor.nodes)
    }
}


#[cfg(test)]
mod test {
    use crate::parser::{Identifier, NodeInstruct, ParserError, Parser, Value};
    use std::collections::BTreeMap;

    struct TestHelper {
        data: BTreeMap<Identifier, NodeInstruct>,
    }

    impl TestHelper {
        pub fn assert_matches(&self, path: &str, expected_pattern: impl Fn(&Value) -> bool) {
            let mut parts = path.split("::");
            let module = parts.next().unwrap();
            let property = parts.next().unwrap();

            let value = self.data.get(&Identifier(module.to_string()))
                .and_then(|value| value.inputs.get(property))
                .expect("Value not found");

            assert!(expected_pattern(value), "Value at {} did not match expected pattern", path);
        }
    }

    macro_rules! parser {
        ($input:expr) => {{
            let data = Parser::new($input)?.evaluate()?;
            TestHelper { data }
        }};
    }

    macro_rules! assert_value {
        ($data:expr, $path:expr, Value::Numeric { value: $val:expr, .. }) => {
            $data.assert_matches($path, |value| {
                matches!(value, Value::Numeric { value, .. } if value == $val)
            })
        };
        ($data:expr, $path:expr, Value::String { value: $val:expr, .. }) => {
            $data.assert_matches($path, |value| {
                matches!(value, Value::String { value, .. } if value == $val)
            })
        };
        ($data:expr, $path:expr, $pattern:pat) => {
            $data.assert_matches($path, |value| matches!(value, $pattern))
        };
    }


    #[test]
    fn primitive_values() -> Result<(), ParserError> {
        let input = r#"
            mock mock {
                string          <- "hello world"
                empty           <- ""
                quotes          <- "abc_\"123\"_def"
                true            <- true
                false           <- false
                number          <- 123
                negative        <- -123
                float           <- 0.123
                negative_float  <- -0.123
            }
        "#;

        let data = parser!(input);

        assert_value!(data, "mock::string", Value::String  { value: "hello world", .. });
        assert_value!(data, "mock::empty", Value::String  { value: "", .. });
        assert_value!(data, "mock::quotes", Value::String  { value: "abc_\\\"123\\\"_def", .. });

        assert_value!(data, "mock::true",   Value::Boolean { value: true, .. });
        assert_value!(data, "mock::false",  Value::Boolean { value: false, .. });

        assert_value!(data, "mock::number", Value::Numeric { value: "123", .. });
        assert_value!(data, "mock::negative", Value::Numeric { value: "-123", .. });
        assert_value!(data, "mock::float",  Value::Numeric { value: "0.123", .. });
        assert_value!(data, "mock::negative_float",  Value::Numeric { value: "-0.123", .. });

        Ok(())
    }

    // #[test]
    // pub fn dependencies() -> Result<(), ParserError> {
    //     let input = r#"
    //         a mock { a <- b::i }
    //         b mock { i <- 3 }
    //     "#;
    //
    //     let expected = r#"
    //         a mock a <- b::i
    //         b mock i <- 3
    //     "#;
    //
    //     println!("{:#?}", parser!(input));
    //     // assert_eq!(parser!(input).to_string(), normalize_whitespace(expected));
    //
    //     Ok(())
    // }
}
