// AUTO-GENERATED from src/reaction/sdk.rs by build.rs.
// Run `cargo check` (or any Cargo build) to regenerate. Do not edit directly.

/** A physical pixel coordinate in the Windows virtual screen coordinate space. */
export interface ScreenPoint {
    /** Horizontal physical pixel coordinate. */
    readonly x: number;
    /** Vertical physical pixel coordinate. */
    readonly y: number;
}

/** A grid defined by the centers of its first and last cells. */
export interface UiGrid {
    /** Center of the first row and column. */
    readonly leftTop: ScreenPoint;
    /** Center of the last row and column; may be above or left of leftTop. */
    readonly rightBottom: ScreenPoint;
    /** Number of rows; its absolute value is used and zero means one. */
    readonly rowCount: number;
    /** Number of columns; its absolute value is used and zero means one. */
    readonly colCount: number;
}

/** Context supplied to the default macro entry point. */
export interface NyraContext {
    /** Identifier of the current macro execution. */
    readonly runId: string;
    /** Queues a message for the Rust host to write during instruction execution. */
    log(message: string): void;
    /** Queues a left click at a physical pixel coordinate. */
    click(point: ScreenPoint): void;
    /** Queues a grid click using one-based indices; zero aliases one and negative indices count backward from the end. */
    click_in_grid(grid: UiGrid, row: number, col: number): void;
}
