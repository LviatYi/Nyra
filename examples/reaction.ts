import type {NyraContext} from "../runtime/context";

export default async function reaction(ctx: NyraContext) {
    await ctx.click({x: 640, y: 360});
}
