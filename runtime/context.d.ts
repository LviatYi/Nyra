// AUTO-GENERATED from src/reaction/sdk_types.rs and sdk.rs by build.rs.
// Run `cargo check` (or any Cargo build) to regenerate. Do not edit directly.

/**
 * A complete synchronous operation, shared by native and debug backends.
 */
export type ActionStep = { 
/**
 * A condition that must complete before the action starts.
 */
before: UiCondition | null, 
/**
 * The input action to perform.
 */
action: InputAction, 
/**
 * An attached action performed only after successful input.
 */
after: AfterAction | null, 
/**
 * Optional description included in the operation log.
 */
message: string | null, };

/**
 * A side action performed after successful input.
 */
export type AfterAction = DelayCondition;

/**
 * Click payload; the enclosing InputAction supplies the kind discriminator.
 */
export type ClickInput = { point: ScreenPoint, repeat: ClickRepeatOption | null, };

/**
 * Options for repeating a click action.
 */
export type ClickRepeatOption = { count: number, intervalMs: number, };

/**
 * A fixed blocking delay, usable before or after an input action.
 */
export type DelayCondition = { kind: "delay", durationMs: number, };

/**
 * Drag payload; the enclosing InputAction supplies the kind discriminator.
 */
export type DragInput = { from: ScreenPoint, to: ScreenPoint, durationMs: number, };

/**
 * A Perception JSON resource that must match before an input action starts.
 */
export type ImageCondition = { kind: "image", path: string, timeoutMs?: number, pollIntervalMs?: number, };

/**
 * An input action independent of its prerequisites and attached actions.
 */
export type InputAction = { "kind": "click" } & ClickInput | { "kind": "drag" } & DragInput;

/**
 * A physical pixel coordinate in the Windows virtual screen coordinate space.
 */
export type ScreenPoint = { 
/**
 * Horizontal physical pixel coordinate.
 */
x: number, 
/**
 * Vertical physical pixel coordinate.
 */
y: number, };

/**
 * A blocking prerequisite for an input action.
 */
export type UiCondition = DelayCondition | ImageCondition;

/** Context supplied to the default macro entry point. */
export interface NyraContext {
    /** Identifier of the current macro execution. */
    readonly runId: string;
    /** Writes a message through the Rust host before returning. */
    log(message: string): void;
    /** Completes a prerequisite, an input action and its attached action before returning. */
    execute(step: ActionStep): void;
    /** Blocks the script for a fixed duration, subject to cancellation and the run timeout. */
    delay(durationMs: number): void;
}
