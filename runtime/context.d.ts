// AUTO-GENERATED from src/reaction/sdk.rs by build.rs.
// Run `cargo check` (or any Cargo build) to regenerate. Do not edit directly.

/** A physical pixel coordinate in the Windows virtual screen coordinate space. */
export interface ScreenPoint {
    /** Horizontal physical pixel coordinate. */
    readonly x: number;
    /** Vertical physical pixel coordinate. */
    readonly y: number;
}

/** A complete synchronous operation, shared by native and debug backends. */
export interface ActionStep {
    /** A condition that must complete before the action starts. */
    readonly before: UiCondition | null;
    /** The input action to perform. */
    readonly action: InputAction;
    /** An attached action performed only after successful input. */
    readonly after: AfterAction | null;
    /** Optional description included in the operation log. */
    readonly message: string | null;
}

export interface DelayCondition {
    readonly kind: "delay";
    readonly durationMs: number;
}

export interface ImageCondition {
    readonly kind: "image";
    readonly path: string;
    readonly timeoutMs?: number;
    readonly pollIntervalMs?: number;
}

/** A blocking prerequisite for an input action. */
type UiCondition =
    | DelayCondition
    | ImageCondition
;

/** A side action performed after successful input. */
export type AfterAction =
    | { readonly kind: "delay"; readonly durationMs: number; }
;

/** An input action independent of its prerequisites and attached actions. */
export type InputAction =
    | { readonly kind: "click"; readonly point: ScreenPoint; }
    | { readonly kind: "drag"; readonly from: ScreenPoint; readonly to: ScreenPoint; readonly durationMs: number; }
;

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
