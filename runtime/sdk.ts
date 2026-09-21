import type {NyraContext as NativeContext, ScreenPoint} from "./context";

export type {ScreenPoint} from "./context";

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

type ClickWithLog = (point: ScreenPoint, message: string) => void;

const i32Min = -2_147_483_648n;
const i32Max = 2_147_483_647n;

function integer(value: number, name: string): bigint {
    if (!Number.isSafeInteger(value)) {
        throw new Error(`${name} must be a safe integer`);
    }
    return BigInt(value);
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

export function createLocalMethods(clickWithLog: ClickWithLog) {
    return Object.freeze({
        click_in_grid(grid: UiGrid, row: number, col: number): void {
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
            clickWithLog(
                point,
                `CLICK IN GRID AT (${row}, ${col}) RESOLVED TO (${point.x}, ${point.y})`,
            );
        },

        click_in_line(
            start: ScreenPoint,
            end: ScreenPoint,
            itemCount: number,
            index: number,
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
            clickWithLog(
                point,
                `CLICK IN LINE AT ${index} RESOLVED TO (${point.x}, ${point.y})`,
            );
        },

        click_in_array(points: readonly ScreenPoint[], index: number): void {
            if (points.length === 0) {
                throw new Error("click_in_array requires at least one point");
            }
            const resolvedIndex = arrayIndex(index, points.length);
            const source = points[resolvedIndex];
            const point = {x: source.x, y: source.y};
            clickWithLog(
                point,
                `CLICK IN ARRAY AT ${index} RESOLVED TO (${point.x}, ${point.y})`,
            );
        },
    });
}

export type LocalContext = ReturnType<typeof createLocalMethods>;
export type NyraContext = Omit<NativeContext, "click_with_log"> & LocalContext;
