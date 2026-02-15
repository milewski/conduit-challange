use crate::dsl::parser::{
    CallbackAssignment, Direction, EVENT_PAYLOAD_IDENTIFIER, EventCallback, Identifier, NodeInstruct, StringPart, Value,
};
use crate::node::SharedValue;
use crate::registry::{NodeRegistry, Payload};
use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};
use std::sync::Arc;
use tokio::sync::mpsc;

pub(super) struct CallbackExecutionContext<'a> {
    pub(super) registry: &'a Arc<NodeRegistry>,
    pub(super) nodes: &'a BTreeMap<Identifier, NodeInstruct>,
    pub(super) input_names: &'a HashSet<String>,
    pub(super) outputs: &'a mut HashMap<Identifier, HashMap<String, SharedValue>>,
    pub(super) event_handlers: &'a BTreeMap<Identifier, BTreeMap<String, Vec<EventCallback>>>,
}

impl<'a> CallbackExecutionContext<'a> {
    pub(super) fn new(
        registry: &'a Arc<NodeRegistry>,
        nodes: &'a BTreeMap<Identifier, NodeInstruct>,
        input_names: &'a HashSet<String>,
        outputs: &'a mut HashMap<Identifier, HashMap<String, SharedValue>>,
        event_handlers: &'a BTreeMap<Identifier, BTreeMap<String, Vec<EventCallback>>>,
    ) -> Self {
        Self {
            registry,
            nodes,
            input_names,
            outputs,
            event_handlers,
        }
    }

    pub(super) async fn execute_node(
        &mut self,
        node_identifier: &Identifier,
        node_instruct: &NodeInstruct,
        payload: Payload,
    ) -> Result<(), crate::node::NodeError> {
        let node_instance = self
            .registry
            .create(&node_instruct.module, Default::default())
            .ok_or_else(|| crate::node::NodeError::ModuleNotFound(node_instruct.module.clone()))?;

        let (event_sender, mut event_receiver) = mpsc::unbounded_channel();
        let mut node_execution_future =
            Box::pin(node_instance.run_with_payload_with_event_sender(payload, Some(event_sender)));
        let mut streamed_event_count = 0usize;
        let mut is_event_stream_open = true;
        let node_event_handlers = self.event_handlers.get(node_identifier);

        let execution_result = loop {
            if is_event_stream_open {
                tokio::select! {
                    maybe_emitted_event = event_receiver.recv() => {
                        if let Some(emitted_event) = maybe_emitted_event {
                            streamed_event_count += 1;
                            Box::pin(self.run_event_callbacks(node_event_handlers, vec![emitted_event])).await?;
                        } else {
                            is_event_stream_open = false;
                        }
                    }
                    execution_result = &mut node_execution_future => {
                        break execution_result?;
                    }
                }
            } else {
                break node_execution_future.await?;
            }
        };

        while let Ok(emitted_event) = event_receiver.try_recv() {
            streamed_event_count += 1;
            Box::pin(self.run_event_callbacks(node_event_handlers, vec![emitted_event])).await?;
        }

        let mut emitted_events = execution_result.events;
        let remaining_emitted_events = if streamed_event_count < emitted_events.len() {
            emitted_events.split_off(streamed_event_count)
        } else {
            Vec::new()
        };

        let node_outputs: HashMap<String, SharedValue> = execution_result
            .outputs
            .into_iter()
            .map(|(output_name, output_value)| (output_name.to_string(), output_value))
            .collect();
        self.outputs.insert(node_identifier.clone(), node_outputs);

        if !remaining_emitted_events.is_empty() {
            Box::pin(self.run_event_callbacks(node_event_handlers, remaining_emitted_events)).await?;
        }

        Ok(())
    }

    async fn run_event_callbacks(
        &mut self,
        event_handlers: Option<&BTreeMap<String, Vec<EventCallback>>>,
        emitted_events: Vec<crate::traits::EventData>,
    ) -> Result<(), crate::node::NodeError> {
        let mut queued_callbacks: VecDeque<(EventCallback, Option<SharedValue>)> = VecDeque::new();

        for emitted_event in emitted_events {
            if let Some(callbacks) = event_handlers.and_then(|handlers| handlers.get(&emitted_event.name)) {
                for callback in callbacks {
                    queued_callbacks.push_back((callback.clone(), emitted_event.value.clone()));
                }
            }
        }

        while let Some((event_callback, event_payload)) = queued_callbacks.pop_front() {
            let payload_value = event_payload.clone().unwrap_or_else(|| Arc::new(()) as SharedValue);

            self.outputs
                .entry(EVENT_PAYLOAD_IDENTIFIER.to_string())
                .or_default()
                .insert("output".to_string(), payload_value);

            match event_callback {
                EventCallback::PipedValue(callback_value) => {
                    self.execute_value_callback(callback_value, event_payload.clone(), true)
                        .await?;
                }
                EventCallback::Value(callback_value) => {
                    self.execute_value_callback(callback_value, event_payload.clone(), false)
                        .await?;
                }
                EventCallback::Assignment(CallbackAssignment {
                    identifier,
                    property,
                    value,
                }) => {
                    self.ensure_callback_dependencies_resolved(&value).await?;
                    let resolved_value =
                        super::resolve_single_value(&value, self.outputs, self.nodes, self.input_names)?;
                    self.outputs
                        .entry(identifier)
                        .or_default()
                        .insert(property, resolved_value);
                }
                EventCallback::Block(callbacks) => {
                    for callback in callbacks {
                        queued_callbacks.push_back((callback, event_payload.clone()));
                    }
                }
            }

            self.outputs.remove(EVENT_PAYLOAD_IDENTIFIER);
        }

        Ok(())
    }

    async fn execute_value_callback(
        &mut self,
        callback_value: Value,
        event_payload: Option<SharedValue>,
        should_pipe_payload: bool,
    ) -> Result<(), crate::node::NodeError> {
        if let Value::Relation {
            identifier, property, ..
        } = callback_value
        {
            let Some(callback_node) = self.nodes.get(&identifier).cloned() else {
                if let Some(payload_value) = event_payload {
                    self.outputs
                        .entry(identifier)
                        .or_default()
                        .insert(property, payload_value);
                }
                return Ok(());
            };

            if callback_node.module == "_" || !self.registry.has(&callback_node.module) {
                let mut visited_identifiers = HashSet::new();
                Box::pin(self.execute_callback_dependencies(&identifier, &mut visited_identifiers)).await?;

                let mut resolved_callback_outputs =
                    super::resolve_inputs(&callback_node, self.outputs, self.nodes, self.input_names)?;

                if should_pipe_payload && let Some(payload_value) = event_payload {
                    resolved_callback_outputs.insert(property, payload_value);
                }

                if !resolved_callback_outputs.is_empty() {
                    self.outputs
                        .entry(identifier)
                        .or_default()
                        .extend(resolved_callback_outputs);
                }
                return Ok(());
            }

            let mut visited_identifiers = HashSet::new();
            Box::pin(self.execute_callback_dependencies(&identifier, &mut visited_identifiers)).await?;

            let mut callback_payload =
                super::resolve_inputs(&callback_node, self.outputs, self.nodes, self.input_names)?;
            if should_pipe_payload && let Some(payload_value) = event_payload {
                callback_payload.insert(property, payload_value);
            }

            self.execute_node(&identifier, &callback_node, callback_payload).await?;

            let mut visited_identifiers = HashSet::new();
            Box::pin(self.execute_output_chained_nodes(&identifier, &mut visited_identifiers)).await?;
            return Ok(());
        }

        let _ = super::resolve_single_value(&callback_value, self.outputs, self.nodes, self.input_names)?;
        Ok(())
    }

    async fn ensure_callback_dependencies_resolved(
        &mut self,
        callback_value: &Value,
    ) -> Result<(), crate::node::NodeError> {
        let mut dependency_identifiers = Vec::new();
        collect_input_dependency_targets(callback_value, &mut dependency_identifiers);

        let mut visited_dependency_identifiers = HashSet::new();

        for dependency_identifier in dependency_identifiers {
            if !visited_dependency_identifiers.insert(dependency_identifier.clone())
                || self.outputs.contains_key(&dependency_identifier)
            {
                continue;
            }

            let Some(dependency_node) = self.nodes.get(&dependency_identifier).cloned() else {
                continue;
            };

            let mut visited_identifiers = HashSet::new();
            Box::pin(self.execute_callback_dependencies(&dependency_identifier, &mut visited_identifiers)).await?;

            if dependency_node.module == "_" || !self.registry.has(&dependency_node.module) {
                let resolved_outputs =
                    super::resolve_inputs(&dependency_node, self.outputs, self.nodes, self.input_names)?;
                if !resolved_outputs.is_empty() {
                    self.outputs
                        .entry(dependency_identifier.clone())
                        .or_default()
                        .extend(resolved_outputs);
                }
                continue;
            }

            let dependency_payload =
                super::resolve_inputs(&dependency_node, self.outputs, self.nodes, self.input_names)?;
            self.execute_node(&dependency_identifier, &dependency_node, dependency_payload)
                .await?;

            let mut visited_identifiers = HashSet::new();
            Box::pin(self.execute_output_chained_nodes(&dependency_identifier, &mut visited_identifiers)).await?;
        }

        Ok(())
    }

    async fn execute_callback_dependencies(
        &mut self,
        node_identifier: &Identifier,
        visited_identifiers: &mut HashSet<Identifier>,
    ) -> Result<(), crate::node::NodeError> {
        if !visited_identifiers.insert(node_identifier.clone()) {
            return Ok(());
        }

        let Some(node_instruct) = self.nodes.get(node_identifier).cloned() else {
            return Ok(());
        };

        let mut dependency_identifiers = Vec::new();
        for input_value in node_instruct.inputs.values() {
            collect_input_dependency_targets(input_value, &mut dependency_identifiers);
        }

        for dependency_identifier in dependency_identifiers {
            if self.outputs.contains_key(&dependency_identifier) {
                continue;
            }

            let Some(dependency_node) = self.nodes.get(&dependency_identifier).cloned() else {
                continue;
            };
            if dependency_node.module == "_" || !self.registry.has(&dependency_node.module) {
                continue;
            }

            Box::pin(self.execute_callback_dependencies(&dependency_identifier, visited_identifiers)).await?;

            let dependency_payload =
                super::resolve_inputs(&dependency_node, self.outputs, self.nodes, self.input_names)?;
            self.execute_node(&dependency_identifier, &dependency_node, dependency_payload)
                .await?;

            Box::pin(self.execute_output_chained_nodes(&dependency_identifier, visited_identifiers)).await?;
        }

        Ok(())
    }

    async fn execute_output_chained_nodes(
        &mut self,
        source_identifier: &Identifier,
        visited_identifiers: &mut HashSet<Identifier>,
    ) -> Result<(), crate::node::NodeError> {
        if !visited_identifiers.insert(source_identifier.clone()) {
            return Ok(());
        }

        let Some(source_node) = self.nodes.get(source_identifier).cloned() else {
            return Ok(());
        };

        let mut output_targets = Vec::new();
        for input_value in source_node.inputs.values() {
            collect_output_targets(input_value, &mut output_targets);
        }

        for target_identifier in output_targets {
            let Some(target_node) = self.nodes.get(&target_identifier).cloned() else {
                continue;
            };
            if target_node.module == "_" || !self.registry.has(&target_node.module) {
                continue;
            }

            let target_payload = super::resolve_inputs(&target_node, self.outputs, self.nodes, self.input_names)?;
            self.execute_node(&target_identifier, &target_node, target_payload)
                .await?;

            Box::pin(self.execute_output_chained_nodes(&target_identifier, visited_identifiers)).await?;
        }

        Ok(())
    }
}

fn collect_output_targets(value: &Value, output_targets: &mut Vec<Identifier>) {
    match value {
        Value::Relation {
            identifier, direction, ..
        } => {
            if *direction == Direction::Output {
                output_targets.push(identifier.clone());
            }
        }
        Value::Tuple { values, .. } => {
            for tuple_value in values {
                collect_output_targets(tuple_value, output_targets);
            }
        }
        _ => {}
    }
}

fn collect_input_dependency_targets(value: &Value, input_dependency_targets: &mut Vec<Identifier>) {
    match value {
        Value::Relation {
            identifier, direction, ..
        } => {
            if *direction == Direction::Input {
                input_dependency_targets.push(identifier.clone());
            }
        }
        Value::String { parts, .. } => {
            for part in parts {
                if let StringPart::Interpolation(expression) = part {
                    for (reference_identifier, _) in super::collect_expression_references(expression) {
                        input_dependency_targets.push(reference_identifier.to_string());
                    }
                }
            }
        }
        Value::Expression { value, .. } => {
            for (reference_identifier, _) in super::collect_expression_references(value) {
                input_dependency_targets.push(reference_identifier.to_string());
            }
        }
        Value::Tuple { values, .. } => {
            for tuple_value in values {
                collect_input_dependency_targets(tuple_value, input_dependency_targets);
            }
        }
        _ => {}
    }
}
