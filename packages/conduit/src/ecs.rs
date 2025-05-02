use crate::dsl::parser::{Direction, Identifier, NodeParser, Value};
use crate::ecs;
use crate::node::SharedValue;
use crate::registry::NodeRegistry;
use crate::traits::{Descriptor, ExecutableNode};
use bevy_ecs::change_detection::{Res, ResMut};
use bevy_ecs::component::Component;
use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::{Commands, Query, Resource, Schedule, With, Without, World};
use std::collections::HashMap;
use std::sync::Arc;

pub struct Engine {
    world: World,
    schedule: Schedule,
    registry: NodeRegistry,
}

impl Engine {
    pub fn new() -> Self {
        Self {
            world: World::new(),
            schedule: Schedule::default(),
            registry: NodeRegistry::new(),
        }
    }

    pub fn run_pipeline(&mut self, workflow: &str) {
        let parser = NodeParser::new(workflow);
        let result = parser.unwrap().evaluate().unwrap();

        // let mut world = World::new();
        // let mut schedule = Schedule::default();
        let mut registry = NodeRegistry::new();

        // Register all nodes automatically using the inventory pattern
        registry.register_all();

        self.world.insert_resource(registry);
        self.schedule.add_systems(ecs::setup);

        let mut inputs = InputsMap::default();

        for (id, instruct) in result.into_iter() {
            self.world.spawn(Module { identifier: id.clone(), name: instruct.module.to_string() });

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

        self.world.insert_resource(inputs);

        // Run the schedule until all nodes are processed
        self.run_until_complete();
    }

    // Function to run the schedule until all nodes are processed
    pub fn run_until_complete(&mut self) {
        let mut iterations = 0;
        let max_iterations = 100; // Safety limit to prevent infinite loops

        loop {
            // Store the count of unprocessed nodes before running the schedule
            let unprocessed_count_before = self.world.query_filtered::<Entity, (With<Module>, Without<Done>)>().iter(&self.world).count();

            if unprocessed_count_before == 0 {
                // All nodes are processed
                break;
            }

            // Run one tick of the schedule
            self.schedule.run(&mut self.world);

            // Check if any progress was made in this iteration
            let unprocessed_count_after = self.world.query_filtered::<Entity, (With<Module>, Without<Done>)>().iter(&self.world).count();

            iterations += 1;

            // If no progress was made and we still have unprocessed nodes, we might have a deadlock
            if unprocessed_count_before == unprocessed_count_after && iterations > 1 {
                // Check if there are any unresolved inputs that could potentially be resolved
                let inputs = self.world.get_resource::<InputsMap>().unwrap();
                if !inputs.has_unresolved_nodes() {
                    println!("Warning: No progress made and no more resolvable inputs. Possible deadlock detected.");
                    break;
                }
            }

            if iterations >= max_iterations {
                println!("Warning: Reached maximum iterations limit ({}). There might be a circular dependency.", max_iterations);
                break;
            }
        }
    }
}

#[derive(Component, Debug)]
pub struct Module {
    identifier: Identifier,
    name: String,
}

#[derive(Component, Debug)]
pub struct Done;

#[derive(Resource, Default)]
struct IdentifierMap(HashMap<Identifier, Entity>);

#[derive(Resource, Default, Debug)]
pub struct InputsMap(HashMap<Identifier, HashMap<String, MaybeResolved>>);

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

    // Check if there are any nodes with unresolved inputs that could potentially be resolved
    pub fn has_unresolved_nodes(&self) -> bool {
        for (_, fields) in self.0.iter() {
            for (_, field) in fields.iter() {
                if let MaybeResolved::Unresolved { .. } = field {
                    return true;
                }
            }
        }

        false
    }
}

#[derive(Debug)]
pub enum MaybeResolved {
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