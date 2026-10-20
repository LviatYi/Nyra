define_sdk! {
    types {
        /// A physical pixel coordinate in the Windows virtual screen coordinate space.
        struct ScreenPoint {
            /// Horizontal physical pixel coordinate.
            x: i32,
            /// Vertical physical pixel coordinate.
            y: i32,
        }
        /// Reserved description of a UI condition evaluated after an action.
        struct UiCondition {
            /// Condition discriminator reserved for the future condition evaluator.
            kind: String,
        }
        /// Complete native options for a click. TypeScript fills every field.
        struct ClickOptions {
            /// Optional message written after a successful click.
            message: Option<String>,
            /// Optional UI condition. Conditions are reserved but not executed yet.
            condition: Option<UiCondition>,
            /// Maximum time reserved for condition evaluation.
            timeout_ms: u64,
            /// Interval reserved for condition polling.
            poll_interval_ms: u64,
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
            /// Performs a left click with complete native options before returning.
            "click"(_execution, run_id, line; point: ScreenPoint, options: Option<ClickOptions>) {
                let message = match options {
                    Some(options) => {
                        if options.timeout_ms == 0 {
                            return Err("click(options.timeoutMs) must be greater than 0".into());
                        }
                        if options.poll_interval_ms == 0 {
                            return Err(
                                "click(options.pollIntervalMs) must be greater than 0".into(),
                            );
                        }
                        if options.poll_interval_ms > options.timeout_ms {
                            return Err(
                                "click(options.pollIntervalMs) cannot exceed timeoutMs".into(),
                            );
                        }
                        if let Some(condition) = options.condition {
                            return Err(format!(
                                "click condition '{}' is reserved but not implemented",
                                condition.kind,
                            ));
                        }
                        if options
                            .message
                            .as_ref()
                            .is_some_and(|message| message.encode_utf16().count() > 2000)
                        {
                            return Err(
                                "click(options.message) length cannot exceed 2000 UTF-16 code units"
                                    .into(),
                            );
                        }
                        options.message
                    }
                    None => None,
                };
                click_screen(&point)?;

                let message = message
                    .unwrap_or_else(|| format!("CLICK AT ({}, {})", point.x, point.y));
                log_in_reaction(run_id, line, &message)
            }
            /// Blocks the script for a fixed duration, subject to cancellation and the run timeout.
            "delay"(execution, run_id, line; duration_ms: u64) {
                execution.delay(std::time::Duration::from_millis(duration_ms))?;
                log_in_reaction(run_id, line, &format!("DELAYED {duration_ms} MS"))
            }
        }
    }
}
