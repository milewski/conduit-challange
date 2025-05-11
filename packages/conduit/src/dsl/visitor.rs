use crate::dsl::error::ParserError;
use crate::dsl::parser::{Direction, Identifier, NodeInstruct, Rule, Value};
use pest::iterators::Pair;
use std::collections::BTreeMap;

#[derive(Debug, Default)]
pub struct Visitor {
    nodes: BTreeMap<Identifier, NodeInstruct>,
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
    pub fn take_nodes(self) -> BTreeMap<Identifier, NodeInstruct> {
        self.nodes
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