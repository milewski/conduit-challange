#![allow(warnings)]

use crate::ecs::Engine;
use bevy_ecs::observer::TriggerTargets;
use bevy_ecs::prelude::*;
use dsl::parser::NodeParser;
use std::any::Any;
use std::io::Read;

mod nodes;
mod registry;
mod traits;
mod node;
mod dsl;
mod ecs;

#[tokio::main()]
async fn main() {
    let pipeline = r#"
        resizer {
            source <- loader { input <- "./me.jpg" }
            width <- 128
            height <- 128
            output -> save { destination <- "me.optimized.jpg" }
        }
    "#;

    let mut engine = Engine::new();

    engine.run_pipeline(pipeline);
}

