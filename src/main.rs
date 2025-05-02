#![allow(warnings)]

use crate::node::SharedValue;
use crate::nodes::loader::Loader;
use crate::nodes::resizer::Resizer;
use crate::nodes::save::Save;
use bevy_ecs::observer::TriggerTargets;
use bevy_ecs::prelude::*;
use std::any::Any;
use std::collections::HashMap;
use std::io::Read;
use std::sync::Arc;

mod nodes;
mod parser;
mod registry;
mod traits;
mod node;

use crate::parser::{Direction, Identifier, Parser, Value};
use crate::registry::NodeRegistry;
use crate::traits::Node;

#[derive(Component, Debug)]
struct Module {
    identifier: Identifier,
    name: String,
}

#[derive(Component, Debug)]
pub struct Done;

#[derive(Resource, Default)]
struct IdentifierMap(HashMap<Identifier, Entity>);

#[derive(Resource, Default, Debug)]
struct InputsMap(HashMap<Identifier, HashMap<String, MaybeResolved>>);

impl InputsMap {
    pub fn is_ready(&self, identifier: &Identifier) -> bool {
        self.0.get(identifier)
            .iter()
            .all(|node| node.iter()
                .filter(|(_, field)| match field {
                    MaybeResolved::Resolved { direction, .. } => *direction == Direction::Input,
                    MaybeResolved::Unresolved { direction, .. } => *direction == Direction::Input
                })
                .all(|(_, field)| matches!(field, MaybeResolved::Resolved { .. }))
            )
    }

    pub fn all_inputs(&self, identifier: &Identifier) -> Option<HashMap<String, SharedValue>> {
        self.0.get(identifier).map(|field| {
            field.iter()
                .filter(|(_, field)| matches!(field, MaybeResolved::Resolved {..}))
                .map(|(name, value)| {
                    match value {
                        MaybeResolved::Resolved { value, .. } => (name.to_string(), value.clone()),
                        MaybeResolved::Unresolved { .. } => unreachable!()
                    }
                })
                .collect()
        })
    }

    pub fn replace_outputs(&mut self, identifier: Identifier, property: &str, value: SharedValue) {
        let relationship = {
            if let Some(inner_map) = self.0.get(&identifier) {
                if let Some(MaybeResolved::Unresolved { identifier, property, .. }) = inner_map.get(property) {
                    Some((identifier.clone(), property.clone()))
                } else {
                    None
                }
            } else {
                None
            }
        };

        self.0.entry(identifier)
            .or_default()
            .entry(property.to_string())
            .and_modify(|field| {
                *field = MaybeResolved::Resolved {
                    direction: Direction::Output,
                    value: value.clone(),
                };
            });

        if let Some((related_identifier, related_property)) = relationship {
            self.replace_outputs(related_identifier, &related_property, value.clone());
        }
    }
}

#[derive(Debug)]
enum MaybeResolved {
    Resolved {
        direction: Direction,
        value: SharedValue,
    },
    Unresolved {
        identifier: Identifier,
        property: String,
        direction: Direction,
    },
}

#[tokio::main()]
async fn main() {
    let parser = Parser::new(
        r#"
            file loader { input <- "./me.jpg" }
            resizer {
                source <- file::output
                width <- 512
                height <- 256
                output -> save { destination <- "example-2.jpg" }
            }
        "#,
    );

    let result = parser.unwrap().evaluate().unwrap();

    let mut world = World::new();
    let mut schedule = Schedule::default();
    let mut registry = NodeRegistry::new();

    registry.register::<Loader>("loader");
    registry.register::<Resizer>("resizer");
    registry.register::<Save>("save");

    world.insert_resource(registry);
    schedule.add_systems(setup);

    let mut inputs = InputsMap::default();

    for (id, instruct) in result.into_iter() {
        world.spawn(Module { identifier: id.clone(), name: instruct.module.to_string() });

        let parent = inputs.0.entry(id).or_default();

        for (name, value) in instruct.inputs.into_iter() {
            parent.entry(name).or_insert_with(|| {
                match value {
                    Value::String { value, direction } => MaybeResolved::Resolved { direction, value: Arc::new(value) },
                    Value::Numeric { value, direction } => MaybeResolved::Resolved { direction, value: Arc::new(value.parse::<u32>().unwrap()) },
                    Value::Boolean { value, direction } => MaybeResolved::Resolved { direction, value: Arc::new(value) },
                    Value::Relation { identifier, property, direction } => MaybeResolved::Unresolved { direction, identifier, property }
                }
            });
        }
    }

    world.insert_resource(inputs);
    schedule.run(&mut world);
    schedule.run(&mut world);
}

pub fn setup(
    mut commands: Commands,
    mut inputs: ResMut<InputsMap>,
    registry: Res<NodeRegistry>,
    query: Query<(Entity, &Module), Without<Done>>,
) {
    for (entity, module) in query {
        if inputs.is_ready(&module.identifier) {
            if let Some(payload) = inputs.all_inputs(&module.identifier) {
                let instance = registry.create(&module.name, payload).unwrap();
                instance.run();
                commands.entity(entity).insert(Done);
                for (attribute, value) in instance.take_outputs() {
                    inputs.replace_outputs(module.identifier.clone(), attribute, value);
                }
            }
        }
    }
}
