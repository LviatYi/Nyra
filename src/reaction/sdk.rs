define_sdk! {
    types {
        /// A physical pixel coordinate in the Windows virtual screen coordinate space.
        struct ScreenPoint {
            /// Horizontal physical pixel coordinate.
            x: i32,
            /// Vertical physical pixel coordinate.
            y: i32,
        }
        /// A complete synchronous operation, shared by native and debug backends.
        struct ActionStep {
            /// A condition that must complete before the action starts.
            before: Option<UiCondition>,
            /// The input action to perform.
            action: InputAction,
            /// An attached action performed only after successful input.
            after: Option<AfterAction>,
            /// Optional description included in the operation log.
            message: Option<String>,
        }
    }
    enums {
        /// A blocking prerequisite for an input action.
        #[sdk(internal)]
        enum UiCondition {
            Delay as DelayCondition { duration_ms: u64 },
            Image as ImageCondition {
                path: String,
                #[serde(default = "default_condition_timeout_ms")]
                timeout_ms: u64,
                #[serde(default = "default_condition_poll_interval_ms")]
                poll_interval_ms: u64,
            },
        }
        /// A side action performed after successful input.
        enum AfterAction {
            Delay { duration_ms: u64 },
        }
        /// An input action independent of its prerequisites and attached actions.
        enum InputAction {
            Click { point: ScreenPoint },
            Drag { from: ScreenPoint, to: ScreenPoint, duration_ms: u64 },
        }
    }
    context NyraContext {
        properties {
            /// Identifier of the current macro execution.
            readonly run_id: String;
        }
        methods {
            /// Writes a message through the Rust host before returning.
            "log"(_execution, run_id, line; message: String) {
                if message.encode_utf16().count() > 2000 {
                    return Err("log(message) length cannot exceed 2000 UTF-16 code units".into());
                }
                log_in_reaction(run_id, line, &message)
            }
            /// Completes a prerequisite, an input action and its attached action before returning.
            "execute"(execution, run_id, line; step: ActionStep) {
                execute_action(execution, run_id, line, step)
            }
            /// Blocks the script for a fixed duration, subject to cancellation and the run timeout.
            "delay"(execution, run_id, line; duration_ms: u64) {
                execution.delay(std::time::Duration::from_millis(duration_ms))?;
                log_in_reaction(run_id, line, &format!("DELAYED {duration_ms} MS"))
            }
        }
    }
}
