// AUTO-GENERATED from src/reaction/sdk.rs by build.rs.
// Run `cargo check` (or any Cargo build) to regenerate. Do not edit directly.

/** An integer coordinate in the Windows virtual screen coordinate space. */
export interface ScreenPoint {
    /** Horizontal screen coordinate. */
    readonly x: number;
    /** Vertical screen coordinate. */
    readonly y: number;
}

/** Context supplied to the default async macro entry point. */
export interface NyraContext {
    /** Identifier of the current macro execution. */
    readonly runId: string;
    /** Writes through the Rust host; await its acknowledgement before continuing. */
    log(message: string): Promise<void>;
    /** Moves the cursor to an integer screen coordinate and performs a left click. */
    click(point: ScreenPoint): Promise<void>;
}
