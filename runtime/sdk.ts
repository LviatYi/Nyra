import type {
    ActionStep,
    AfterAction,
    DelayCondition,
    ImageCondition,
    InputAction,
    NyraContext as NativeContext,
    ScreenPoint,
} from "./context";

export type {ActionStep, AfterAction, DelayCondition, ImageCondition, InputAction, ScreenPoint} from "./context";

/** Image resources are Perception JSON files relative to the configuration directory. */
export type UiCondition = DelayCondition | ImageCondition;

/** Undefined inherits the context default; null explicitly disables a phase. */
export interface ActionOptions {
    readonly before?: UiCondition | null;
    readonly after?: AfterAction | null;
}

export type ClickOptions = ActionOptions;

export interface DragOptions extends ActionOptions {
    /** Time spent moving while holding the left button. Defaults to 300 ms. */
    readonly durationMs?: number;
}

/** Backends consume complete steps; the public SDK owns defaults and coordinate helpers. */
export type ReactionBackend = NativeContext;

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

type Execute = ReactionBackend["execute"];

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

function completeBefore(condition: UiCondition | null | undefined): UiCondition | null {
    if (condition == null) return null;
    if (condition.kind === "delay") {
        return Object.freeze({
            kind: "delay",
            durationMs: milliseconds(condition.durationMs, "before.durationMs", true),
        });
    }
    if (condition.kind !== "image") throw new Error("Unsupported prerequisite kind");
    if (typeof condition.path !== "string" || condition.path.trim().length === 0) {
        throw new Error("Image condition path cannot be empty");
    }
    const timeoutMs = milliseconds(condition.timeoutMs ?? defaultTimeoutMs, "timeoutMs", false);
    const pollIntervalMs = milliseconds(
        condition.pollIntervalMs ?? defaultPollIntervalMs,
        "pollIntervalMs",
        false,
    );
    if (pollIntervalMs > timeoutMs) {
        throw new Error("pollIntervalMs cannot exceed timeoutMs");
    }
    return Object.freeze({kind: "image", path: condition.path, timeoutMs, pollIntervalMs});
}

function completeAfter(action: AfterAction | null | undefined): AfterAction | null {
    if (action == null) return null;
    if (action.kind !== "delay") throw new Error("Unsupported attached action kind");
    return Object.freeze({
        kind: "delay",
        durationMs: milliseconds(action.durationMs, "after.durationMs", true),
    });
}

function copyPoint(point: ScreenPoint): ScreenPoint {
    const x = integer(point.x, "point.x");
    const y = integer(point.y, "point.y");
    if (x < i32Min || x > i32Max || y < i32Min || y > i32Max) {
        throw new Error("Point is outside the supported coordinate range");
    }
    return Object.freeze({x: point.x, y: point.y});
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

export function createLocalMethods(execute: Execute) {
    let defaults: Pick<ActionStep, "before" | "after"> = {
        before: null,
        after: Object.freeze({kind: "delay", durationMs: 50}),
    };

    function performAction(action: InputAction, message: string | null, options?: ActionOptions): void {
        const before = options?.before === undefined ? defaults.before : completeBefore(options.before);
        const after = options?.after === undefined ? defaults.after : completeAfter(options.after);
        execute(Object.freeze({before, action: Object.freeze(action), after, message}));
    }

    function performClick(
        point: ScreenPoint,
        message: string | null,
        options?: ClickOptions,
    ): void {
        performAction({kind: "click", point: copyPoint(point)}, message, options);
    }

    return Object.freeze({
        /** Updates subsequent operations in this context. Omitted phases keep their defaults. */
        set_defaults(options: ActionOptions): void {
            const before = options.before === undefined ? defaults.before : completeBefore(options.before);
            const after = options.after === undefined ? defaults.after : completeAfter(options.after);
            defaults = {before, after};
        },

        click(point: ScreenPoint, options?: ClickOptions): void {
            performClick(point, null, options);
        },

        drag(from: ScreenPoint, to: ScreenPoint, options?: DragOptions): void {
            const durationMs = milliseconds(options?.durationMs ?? 300, "drag.durationMs", true);
            performAction({kind: "drag", from: copyPoint(from), to: copyPoint(to), durationMs}, null, options);
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
            performClick(
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
            performClick(
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
            performClick(
                point,
                `CLICK IN ARRAY AT ${index} RESOLVED TO (${point.x}, ${point.y})`,
                options,
            );
        },
    });
}

export type LocalContext = ReturnType<typeof createLocalMethods>;
export type NyraContext = Omit<NativeContext, "execute"> & LocalContext;

/** Creates isolated defaults for one macro invocation, regardless of the execution backend. */
export function createContext(backend: ReactionBackend): NyraContext {
    return Object.freeze({
        runId: backend.runId,
        log(message: string): void {
            if (typeof message !== "string" || message.length > 2000) {
                throw new Error("log(message) must be a string of at most 2000 UTF-16 code units");
            }
            backend.log(message);
        },
        delay(durationMs: number): void {
            backend.delay(milliseconds(durationMs, "durationMs", true));
        },
        ...createLocalMethods((step) => backend.execute(step)),
    });
}

export interface DebugMarker {
    readonly sequence: number;
    readonly step: ActionStep;
}

export interface DebugRenderer {
    /** Draw a numbered click, or a drag path, using the complete resolved step. */
    mark(marker: DebugMarker): void;
    log?(message: string): void;
    delay?(durationMs: number): void;
}

/** Preview backend: emits markers without sending input, capturing the screen or sleeping. */
export function createDebugContext(renderer: DebugRenderer, runId = "debug"): NyraContext {
    let sequence = 0;
    return createContext({
        runId,
        execute(step): void {
            renderer.mark(Object.freeze({sequence: ++sequence, step}));
        },
        log(message): void {
            renderer.log?.(message);
        },
        delay(durationMs): void {
            renderer.delay?.(durationMs);
        },
    });
}
