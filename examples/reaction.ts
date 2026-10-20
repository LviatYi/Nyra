import type { NyraContext } from "../runtime/sdk";

export default function reaction(ctx: NyraContext) {
    // Applies to subsequent clicks, click helpers and drags in this invocation.
    ctx.set_defaults({
        before: {kind: "delay", durationMs: 100},
        after: {kind: "delay", durationMs: 50},
    });
    ctx.click({x: 640, y: 360});
    // A one-off override does not change the defaults used by the next operation.
    ctx.click_in_line({x: 640, y: 360}, {x: 640, y: 400}, 3, 2, {before: null});
    ctx.click({x: 840, y: 360});
    ctx.click_in_line({x: 640, y: 360}, {x: 640, y: 400}, 3, 2);
    ctx.click({x: 1240, y: 360});
    ctx.click_in_line({x: 640, y: 360}, {x: 640, y: 400}, 3, 2);
    ctx.click({x: 1640, y: 360});
    ctx.click_in_line({x: 640, y: 360}, {x: 640, y: 400}, 3, 2);
}
