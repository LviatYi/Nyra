// AUTO-GENERATED from src/reaction/sdk.rs by build.rs.
// Run `cargo check` (or any Cargo build) to regenerate. Do not edit directly.

/** A physical pixel coordinate in the Windows virtual screen coordinate space. */
export interface ScreenPoint {
    /** Horizontal physical pixel coordinate. */
    readonly x: number;
    /** Vertical physical pixel coordinate. */
    readonly y: number;
}

/** Context supplied to the default macro entry point. */
export interface NyraContext {
    /** Identifier of the current macro execution. */
    readonly runId: string;
    /** Queues a message for the Rust host to write during instruction execution. */
    log(message: string): void;
    /** Queues a left click at a physical pixel coordinate. */
    click(point: ScreenPoint): void;
    /** Queues a left click and writes the supplied message after it succeeds. */
    click_with_log(point: ScreenPoint, message: string): void;
}
