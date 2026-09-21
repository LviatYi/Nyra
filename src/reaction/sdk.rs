define_sdk! {
    types {
        /// A physical pixel coordinate in the Windows virtual screen coordinate space.
        struct ScreenPoint {
            /// Horizontal physical pixel coordinate.
            x: i32,
            /// Vertical physical pixel coordinate.
            y: i32,
        }
    }
    context NyraContext {
        properties {
            /// Identifier of the current macro execution.
            readonly run_id: String;
        }
        methods {
            /// Queues a message for the Rust host to write during instruction execution.
            "log"(run_id, line; message: String) {
                if message.encode_utf16().count() > 2000 {
                    return Err("log(message) length cannot exceed 2000 UTF-16 code units".into());
                }

                log_in_reaction(run_id, line, &message)
            }
            /// Queues a left click at a physical pixel coordinate.
            "click"(run_id, line; point: ScreenPoint) {
                click_screen(&point)?;

                log_in_reaction(
                    run_id,
                    line,
                    &format!("CLICK AT ({}, {})", point.x, point.y),
                )
            }

            /// Queues a left click and writes the supplied message after it succeeds.
            "click_with_log"(run_id, line; point: ScreenPoint, message: String) {
                if message.encode_utf16().count() > 2000 {
                    return Err(
                        "click_with_log(message) length cannot exceed 2000 UTF-16 code units"
                            .into(),
                    );
                }
                click_screen(&point)?;
                log_in_reaction(run_id, line, &message)
            }
        }
    }
}
