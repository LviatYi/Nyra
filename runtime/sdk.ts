import type {
    ClickOptions as NativeClickOptions,
    NyraContext as NativeContext,
    ScreenPoint,
    UiCondition,
} from "./context";

export type {ScreenPoint, UiCondition} from "./context";

/** User-facing click options. Omitted values are filled before serialization. */
export interface ClickOptions {
    /** Fixed delay emitted as a separate instruction after the click. Defaults to 100 ms. */
    readonly delayMs?: number;
    /** Reserved UI condition evaluated by Rust after the click. */
    readonly condition?: UiCondition | null;
    /** Condition timeout. Defaults to 3000 ms. */
    readonly timeoutMs?: number;
    /** Condition polling interval. Defaults to 50 ms. */
    readonly pollIntervalMs?: number;
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

type NativeClick = (point: ScreenPoint, options: NativeClickOptions) => void;
type NativeDelay = (durationMs: number) => void;

const defaultDelayMs = 50;
const defaultTimeoutMs = 3_000;
const defaultPollIntervalMs = 50;

const i32Min = -2_147_483_648n;
const i32Max = 2_147_483_647n;

function integer(value: number, name: string): bigint {
    if (!Number.isSafeInteger(value)) {
        throw new Error(`${name} must be a safe integer`);
    }
    return BigInt(value);
}

function milliseconds(value: number, name: string, allowZero: boolean): number {
    if (!Number.isSafeInteger(value) || value < 0 || (!allowZero && value === 0)) {
        const range = allowZero ? "a non-negative safe integer" : "a positive safe integer";
        throw new Error(`${name} must be ${range}`);
    }
    return value;
}

function completeClickOptions(
    options: ClickOptions | undefined,
    message: string | null,
): {native: NativeClickOptions; delayMs: number} {
    const delayMs = milliseconds(options?.delayMs ?? defaultDelayMs, "delayMs", true);
    const timeoutMs = milliseconds(options?.timeoutMs ?? defaultTimeoutMs, "timeoutMs", false);
    const pollIntervalMs = milliseconds(
        options?.pollIntervalMs ?? defaultPollIntervalMs,
        "pollIntervalMs",
        false,
    );
    if (pollIntervalMs > timeoutMs) {
        throw new Error("pollIntervalMs cannot exceed timeoutMs");
    }
    return {
        native: {
            message,
            condition: options?.condition ?? null,
            timeoutMs,
            pollIntervalMs,
        },
        delayMs,
    };
}

function count(value: number, name: string): bigint {
    const valueAsInteger = integer(value, name);
    const absolute = valueAsInteger < 0n ? -valueAsInteger : valueAsInteger;
    return absolute === 0n ? 1n : absolute;
}

function axisCoordinate(
    start: number,
    end: number,
    itemCount: number,
    index: number,
    axis: string,
): number {
    const startAsInteger = integer(start, `${axis} start`);
    const endAsInteger = integer(end, `${axis} end`);
    const normalizedCount = count(itemCount, `${axis} count`);
    const indexAsInteger = integer(index, `${axis} index`);
    const position = indexAsInteger > 0n ? indexAsInteger - 1n : indexAsInteger;
    const coordinate = startAsInteger === endAsInteger || normalizedCount === 1n
        ? startAsInteger
        : startAsInteger
            + (endAsInteger - startAsInteger) * position / (normalizedCount - 1n);
    if (coordinate < i32Min || coordinate > i32Max) {
        throw new Error(`Resolved ${axis} coordinate is outside the supported range`);
    }
    return Number(coordinate);
}

function arrayIndex(index: number, length: number): number {
    const indexAsInteger = integer(index, "array index");
    const lengthAsInteger = BigInt(length);
    const resolved = indexAsInteger > 0n
        ? indexAsInteger - 1n
        : indexAsInteger === 0n
            ? 0n
            : lengthAsInteger + indexAsInteger;
    if (resolved < 0n || resolved >= lengthAsInteger) {
        throw new Error(`click_in_array index ${index} is out of range`);
    }
    return Number(resolved);
}

export function createLocalMethods(clickNative: NativeClick, delayNative: NativeDelay) {
    function enqueueClick(
        point: ScreenPoint,
        message: string | null,
        options?: ClickOptions,
    ): void {
        const complete = completeClickOptions(options, message);
        clickNative(point, complete.native);
        if (complete.delayMs > 0) {
            delayNative(complete.delayMs);
        }
    }

    return Object.freeze({
        click(point: ScreenPoint, options?: ClickOptions): void {
            enqueueClick(point, null, options);
        },

        click_in_grid(grid: UiGrid, row: number, col: number, options?: ClickOptions): void {
            const point = {
                x: axisCoordinate(
                    grid.leftTop.x,
                    grid.rightBottom.x,
                    grid.colCount,
                    col,
                    "grid x",
                ),
                y: axisCoordinate(
                    grid.leftTop.y,
                    grid.rightBottom.y,
                    grid.rowCount,
                    row,
                    "grid y",
                ),
            };
            enqueueClick(
                point,
                `CLICK IN GRID AT (${row}, ${col}) RESOLVED TO (${point.x}, ${point.y})`,
                options,
            );
        },

        click_in_line(
            start: ScreenPoint,
            end: ScreenPoint,
            itemCount: number,
            index: number,
            options?: ClickOptions,
        ): void {
            const horizontal = start.y === end.y && start.x !== end.x;
            const vertical = start.x === end.x && start.y !== end.y;
            if (!horizontal && !vertical) {
                throw new Error(
                    "click_in_line requires distinct start and end points on one horizontal or vertical line",
                );
            }
            const point = horizontal
                ? {
                    x: axisCoordinate(start.x, end.x, itemCount, index, "line x"),
                    y: start.y,
                }
                : {
                    x: start.x,
                    y: axisCoordinate(start.y, end.y, itemCount, index, "line y"),
                };
            enqueueClick(
                point,
                `CLICK IN LINE AT ${index} RESOLVED TO (${point.x}, ${point.y})`,
                options,
            );
        },

        click_in_array(
            points: readonly ScreenPoint[],
            index: number,
            options?: ClickOptions,
        ): void {
            if (points.length === 0) {
                throw new Error("click_in_array requires at least one point");
            }
            const resolvedIndex = arrayIndex(index, points.length);
            const source = points[resolvedIndex];
            const point = {x: source.x, y: source.y};
            enqueueClick(
                point,
                `CLICK IN ARRAY AT ${index} RESOLVED TO (${point.x}, ${point.y})`,
                options,
            );
        },
    });
}

export type LocalContext = ReturnType<typeof createLocalMethods>;
export type NyraContext = Omit<NativeContext, "click"> & LocalContext;
