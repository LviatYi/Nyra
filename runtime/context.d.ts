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
 * A screen region or a window selected by executable filename first, then exact window title.
 */
export type CaptureTarget = { "mode": "screen", monitor: MonitorConfig, region: Region, } | { "mode": "window", 
/**
 * Matches executable filenames (case-insensitive) before exact window titles.
 */
title: string, region: WindowRegion, };

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
 * An image match configured directly in TypeScript, used before an input action.
 */
export type ImageCondition = { kind: "image", target: CaptureTarget, 
/**
 * Template image path, relative to the main configuration directory unless absolute.
 */
template: string, 
/**
 * Minimum similarity in [0, 1], defaulting to 0.95.
 */
threshold?: number, timeoutMs?: number, pollIntervalMs?: number, };

/**
 * An input action independent of its prerequisites and attached actions.
 */
export type InputAction = { "kind": "click" } & ClickInput | { "kind": "drag" } & DragInput;

export type MonitorConfig = { 
/**
 * Omitted or null selects the user's primary monitor.
 */
name?: string | null, width: number, height: number, };

export type PositionUnit = "pixels" | "ratio";

/**
 * A fixed region measured in physical pixels from the monitor's top-left corner.
 */
export type Region = { x: number, y: number, width: number, height: number, };

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

/**
 * Position relative to the window's top-left corner; dimensions always remain physical pixels.
 */
export type WindowRegion = { x: number, y: number, width: number, height: number, 
/**
 * Defaults to pixels. Ratio positions multiply x/y by the current window width/height.
 */
unit?: PositionUnit, };

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
