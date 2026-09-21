import type {NyraContext} from "../runtime/sdk";

export default function reaction(ctx: NyraContext) {
    ctx.click({x: 640, y: 360});
    ctx.click_in_line({x: 640, y: 360}, {x: 640, y: 400}, 2, 1);
}
