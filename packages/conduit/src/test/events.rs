use crate::functional_node;
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
fn test_functional_node_can_emit_events() {
    #[derive(NodeEvent)]
    enum FunctionalEvents {
        Done { value: u32 },
    }

    #[functional_node]
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

    #[functional_node]
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

    #[functional_node]
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
            on done current -> {
                store::counter <- current
            }
        }

        <- store::counter
    "#};

    assert_eq!(output, 2);
}
