use crate::pipeline;

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
