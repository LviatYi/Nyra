define_sdk! {
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
