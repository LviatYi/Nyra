define_sdk! {
    types {
        /// A physical pixel coordinate in the Windows virtual screen coordinate space.
        struct ScreenPoint {
            /// Horizontal physical pixel coordinate.
            x: i32,
            /// Vertical physical pixel coordinate.
            y: i32,
        }

        /// A grid defined by the centers of its first and last cells.
        struct UiGrid {
            /// Center of the first row and column.
            left_top: ScreenPoint,
            /// Center of the last row and column; may be above or left of leftTop.
            right_bottom: ScreenPoint,
            /// Number of rows; its absolute value is used and zero means one.
            row_count: i32,
            /// Number of columns; its absolute value is used and zero means one.
            col_count: i32,
        }
    }
    context NyraContext {
        properties {
            /// Identifier of the current macro execution.
            readonly run_id: String;
        }
        methods {
            /// Queues a message for the Rust host to write during instruction execution.
            "log"(run_id, line; message: String) {
                if message.encode_utf16().count() > 2000 {
                    return Err("log(message) length cannot exceed 2000 UTF-16 code units".into());
                }

                log_in_reaction(run_id, line, &message)
            }
            /// Queues a left click at a physical pixel coordinate.
            "click"(run_id, line; point: ScreenPoint) {
                click_screen(&point)?;

                log_in_reaction(
                    run_id,
                    line,
                    &format!("CLICK AT ({}, {})", point.x, point.y),
                )
            }

            /// Queues a grid click using one-based indices; zero aliases one and negative indices count backward from the end.
            "click_in_grid"(run_id, line; grid: UiGrid, row: i32, col: i32) {
                let resolve_axis = |
                    start: i32,
                    end: i32,
                    count: i32,
                    index: i32,
                    axis: &str,
                | -> Result<i32, String> {
                    let count = i64::from(count).abs().max(1);
                    let position = match index.cmp(&0) {
                        std::cmp::Ordering::Greater => i64::from(index) - 1,
                        std::cmp::Ordering::Equal => 0,
                        std::cmp::Ordering::Less => count + i64::from(index),
                    };
                    let coordinate = if count == 1 {
                        i128::from(start)
                    } else {
                        i128::from(start)
                            + (i128::from(end) - i128::from(start)) * i128::from(position)
                                / i128::from(count - 1)
                    };
                    i32::try_from(coordinate).map_err(|_| {
                        format!("Resolved grid {axis} coordinate is outside the supported range")
                    })
                };
                let grid = &grid;
                let point = ScreenPoint {
                    x: resolve_axis(
                        grid.left_top.x,
                        grid.right_bottom.x,
                        grid.col_count,
                        col,
                        "x",
                    )?,
                    y: resolve_axis(
                        grid.left_top.y,
                        grid.right_bottom.y,
                        grid.row_count,
                        row,
                        "y",
                    )?,
                };

                click_screen(&point)?;

                log_in_reaction(
                    run_id,
                    line,
                    &format!("CLICK IN GRID AT ({row}, {col}) RESOLVED TO ({}, {}) IN [({},{}),({},{})]",
                        point.x,
                        point.y,
                        grid.left_top.x,
                        grid.left_top.y,
                        grid.right_bottom.x,
                        grid.right_bottom.y
                    ),
                )
            }
        }
    }
}
