// AUTO-GENERATED from src/reaction/sdk.rs by build.rs.
// Run `cargo check` (or any Cargo build) to regenerate. Do not edit directly.

/** A physical pixel coordinate in the Windows virtual screen coordinate space. */
export interface ScreenPoint {
    /** Horizontal physical pixel coordinate. */
    readonly x: number;
    /** Vertical physical pixel coordinate. */
    readonly y: number;
}

/** Reserved description of a UI condition evaluated after an action. */
export interface UiCondition {
    /** Condition discriminator reserved for the future condition evaluator. */
    readonly kind: string;
}

/** Complete native options for a click. TypeScript fills every field. */
export interface ClickOptions {
    /** Optional message written after a successful click. */
    readonly message: string | null;
    /** Optional UI condition. Conditions are reserved but not executed yet. */
    readonly condition: UiCondition | null;
    /** Maximum time reserved for condition evaluation. */
    readonly timeoutMs: number;
    /** Interval reserved for condition polling. */
    readonly pollIntervalMs: number;
}

/** Context supplied to the default macro entry point. */
export interface NyraContext {
    /** Identifier of the current macro execution. */
    readonly runId: string;
    /** Writes a message through the Rust host before returning. */
    log(message: string): void;
    /** Performs a left click with complete native options before returning. */
    click(point: ScreenPoint, options: ClickOptions | null): void;
    /** Blocks the script for a fixed duration, subject to cancellation and the run timeout. */
    delay(durationMs: number): void;
}
