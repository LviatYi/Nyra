define_sdk! {
    types {
        /// An integer coordinate in the Windows virtual screen coordinate space.
        struct ScreenPoint {
            /// Horizontal screen coordinate.
            x: i32,
            /// Vertical screen coordinate.
            y: i32,
        }
    }
    context NyraContext {
        properties {
            /// Identifier of the current macro execution.
            readonly run_id: String;
        }
        methods {
            /// Writes through the Rust host; await its acknowledgement before continuing.
            "log"(run_id; message: String) {
                if message.encode_utf16().count() > 2000 {
                    return Err("log(message) length cannot exceed 2000 UTF-16 code units".into());
                }

                log_in_reaction(run_id, &message)
            }
            /// Moves the cursor to an integer screen coordinate and performs a left click.
            "click"(run_id; point: ScreenPoint) {
                click_screen(&point)?;

                log_in_reaction(run_id, &format!("CLICK AT ({}, {})", point.x, point.y))
            }
        }
    }
}
