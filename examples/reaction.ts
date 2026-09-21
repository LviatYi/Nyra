import type {NyraContext} from "../runtime/sdk";

export default function reaction(ctx: NyraContext) {
    ctx.click({x: 640, y: 360});
    ctx.click_in_line({x: 640, y: 360}, {x: 640, y: 400}, 3, 2);
    ctx.click({x: 840, y: 360});
    ctx.click_in_line({x: 640, y: 360}, {x: 640, y: 400}, 3, 2);
    ctx.click({x: 1240, y: 360});
    ctx.click_in_line({x: 640, y: 360}, {x: 640, y: 400}, 3, 2);
    ctx.click({x: 1640, y: 360});
    ctx.click_in_line({x: 640, y: 360}, {x: 640, y: 400}, 3, 2);
}
