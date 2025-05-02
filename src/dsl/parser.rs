use std::collections::BTreeMap;
use pest_derive::Parser;
use pest::iterators::Pair;
use pest::Parser;
use bevy_ecs::component::Component;
use uuid::Uuid;
use std::fmt::{Display, Formatter};
use std::ops::Deref;
use crate::dsl::error::ParserError;
use crate::dsl::visitor::Visitor;
#[derive(Parser, Debug)]
#[grammar = "schema.pest"]
pub struct NodeParser {
    inner: Pair<'static, Rule>,
}

impl NodeParser {
    pub fn new(input: &str) -> Result<Self, ParserError> {
        // Convert the input to a 'static lifetime - this is safe because we're parsing it immediately
        // and not storing references to the original string
        let input_static: &'static str = Box::leak(input.to_string().into_boxed_str());

        Ok(NodeParser {
            inner: NodeParser::parse(Rule::nodes, input_static)?.next().expect("invalid input..."),
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

        Ok(visitor.take_nodes())
    }
}


#[cfg(test)]
mod test {
    use crate::parser::Parser;
    use std::collections::BTreeMap;
    use crate::dsl::error::ParserError;
    use crate::dsl::parser::{Identifier, NodeInstruct, Value};

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
    pub fn new(module: &str, identifier: Option<&str>) -> Self {
        NodeInstruct {
            identifier: identifier
                .map(|identifier| Identifier(identifier.to_string()))
                .unwrap_or_else(|| Identifier(Uuid::new_v4().to_string())),
            module: module.to_string(),
            inputs: BTreeMap::default(),
        }
    }
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

    fn as_symbol(self) -> &'static str {
        match self {
            Direction::Input => "<-",
            Direction::Output => "->",
        }
    }
}