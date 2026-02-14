use crate::functional_node;
use crate::pipeline;
use crate::traits::Emitter;
use conduit_derive::NodeEvent;

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
