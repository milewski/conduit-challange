use crate::node;
use crate::pipeline;
use crate::traits::Emitter;
use conduit_derive::NodeEvent;
use std::sync::atomic::{AtomicBool, Ordering};

static CALLBACK_STARTED_EVENT: AtomicBool = AtomicBool::new(false);

#[test]
fn test_events() {
    let output: u32 = pipeline! {r#"
        store _ {
            counter <- 0
        }

        task {
            count <- 2
            on done -> {
                store::counter <- (store::counter + 1)
            }
        }

        <- store::counter
    "#};

    assert_eq!(output, 2);
}

#[test]
fn test_event_lists() {
    let output: u32 = pipeline! {r#"
        task {
            count <- 1
            on [complete, error, message] -> echo
        }

        <- echo
    "#};

    assert_eq!(output, 30);
}

#[test]
fn test_event_callback_explicit_input_target() {
    let output: u32 = pipeline! {r#"
        task {
            count <- 1
            on complete -> echo::input
        }

        <- echo
    "#};

    assert_eq!(output, 10);
}

#[test]
fn test_node_can_emit_events() {
    #[derive(NodeEvent)]
    enum FunctionalEvents {
        Done { value: u32 },
    }

    #[node]
    async fn functional_task_with_emitter(count: u32, emitter: Emitter<FunctionalEvents>) {
        for _ in 0..count {
            emitter.emit(FunctionalEvents::Done { value: 1 }).await;
        }
    }

    let output: u32 = pipeline! {r#"
        store _ {
            counter <- 0
        }

        functional_task_with_emitter {
            count <- 3
            on done -> {
                store::counter <- (store::counter + 1)
            }
        }

        <- store::counter
    "#};

    assert_eq!(output, 3);
}

#[test]
fn test_event_callbacks_execute_while_emitter_is_running() {
    #[derive(NodeEvent)]
    enum StreamingEvent {
        Started,
    }

    #[node]
    async fn emit_and_wait(emitter: Emitter<StreamingEvent>) -> Result<u32, String> {
        CALLBACK_STARTED_EVENT.store(false, Ordering::SeqCst);
        emitter.emit(StreamingEvent::Started).await;

        for _ in 0..20 {
            if CALLBACK_STARTED_EVENT.load(Ordering::SeqCst) {
                return Ok(1);
            }
            tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
        }

        Err("event callback did not run before emitter completed".to_string())
    }

    #[node]
    fn mark_started() {
        CALLBACK_STARTED_EVENT.store(true, Ordering::SeqCst);
    }

    let output: u32 = pipeline! {r#"
        waiter emit_and_wait {
            on started -> mark_started
        }

        <- waiter
    "#};

    assert_eq!(output, 1);
}

#[test]
fn test_event_callback_uses_implicit_payload_name() {
    let output: u32 = pipeline! {r#"
        store _ {
            counter <- 0
        }

        task {
            count <- 2
            on done -> {
                store::counter <- done
            }
        }

        <- store::counter
    "#};

    assert_eq!(output, 2);
}

#[test]
fn test_event_callback_uses_explicit_payload_alias() {
    let output: u32 = pipeline! {r#"
        store _ {
            counter <- 0
        }

        task {
            count <- 2
            on done current {
                store::counter <- current
            }
        }

        <- store::counter
    "#};

    assert_eq!(output, 2);
}

#[test]
fn test_parenthesized_nodes_run_sequentially() {
    let output: u32 = pipeline! {r#"
        store _ {
            counter <- 0
        }

        (
            first task {
                count <- 1
                on done -> {
                    store::counter <- 1
                }
            }

            second task {
                count <- 1
                on done -> {
                    store::counter <- (store::counter * 10)
                }
            }
        )

        <- store::counter
    "#};

    assert_eq!(output, 10);
}

#[test]
fn test_block_callback_node_does_not_receive_event_payload_implicitly() {
    #[node]
    fn as_text(#[input] input: String) -> String {
        input
    }

    let output: u32 = pipeline! {r#"
        store _ {
            counter <- 0
        }

        task {
            count <- 1
            on done value {
                latest_text as_text {
                    <- "ok"
                }
                store::counter <- value
            }
        }

        <- store::counter
    "#};

    assert_eq!(output, 1);
}

#[test]
fn test_nested_event_payload_aliases_are_captured_correctly() {
    #[derive(NodeEvent)]
    enum PromptEvent {
        Answer { value: u32 },
    }

    #[node]
    async fn first_prompt(emitter: Emitter<PromptEvent>) {
        emitter.emit(PromptEvent::Answer { value: 5 }).await;
    }

    #[node]
    async fn second_prompt(emitter: Emitter<PromptEvent>) {
        emitter.emit(PromptEvent::Answer { value: 8 }).await;
    }

    let output: (u32, u32) = pipeline! {r#"
        store _ {
            width <- 0
            height <- 0
        }

        first_prompt {
            on answer width {
                second_prompt {
                    on answer height {
                        store::width <- width
                        store::height <- height
                    }
                }
            }
        }

        <- store::width
        <- store::height
    "#};

    assert_eq!(output, (5, 8));
}

#[test]
fn test_callback_block_executes_nested_output_chain() {
    let output: u32 = pipeline! {r#"
        task {
            count <- 1
            on done value {
                adder {
                    a <- value
                    b <- 2
                    -> echo {}
                }
            }
        }

        <- 1
    "#};

    assert_eq!(output, 1);
}

#[test]
fn test_callback_block_executes_input_dependencies_before_node() {
    #[node]
    fn produce_value() -> u32 {
        7
    }

    #[node]
    fn consume_value(#[input] input: u32) -> u32 {
        input
    }

    let output: u32 = pipeline! {r#"
        store _ {
            seen <- 0
        }

        task {
            count <- 1
            on done value {
                result consume_value {
                    <- produce_value {}
                }
                store::seen <- result
            }
        }

        <- store::seen
    "#};

    assert_eq!(output, 7);
}

#[test]
fn test_callback_block_data_node_is_resolved_before_downstream_dependency() {
    let output: (u32, u32) = pipeline! {r#"
        task {
            count <- 1
            on done width {
                task {
                    count <- 1
                    on done height {
                        config _ {
                            width <- width
                            height <- height
                        }
                    }
                }
            }
        }

        <- config::width
        <- config::height
    "#};

    assert_eq!(output, (1, 1));
}

#[test]
fn test_event_payload_alias_can_be_interpolated_in_string() {
    let output: String = pipeline! {r#"
        store _ {
            message <- ""
        }

        task {
            count <- 3
            on done width {
                store::message <- "Awesome the width was: { width }, how about the height?"
            }
        }

        <- store::message
    "#};

    assert_eq!(output, "Awesome the width was: 3, how about the height?");
}

#[test]
fn test_callback_block_data_node_executes_inline_node_before_resolution() {
    #[node]
    fn convert_to_number(#[input] input: String) -> Result<u32, String> {
        input.parse::<u32>().map_err(|error| error.to_string())
    }

    let output: u32 = pipeline! {r#"
        config _ {
            age <- "20"
        }

        task {
            count <- 1
            on done {
                output _ {
                    age <- convert_to_number <- config::age
                }
            }
        }

        <- output::age
    "#};

    assert_eq!(output, 20);
}
