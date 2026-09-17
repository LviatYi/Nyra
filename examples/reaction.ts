import type {NyraContext} from "../runtime/context";

export default async function reaction(ctx: NyraContext) {
    await ctx.log(`Reaction start: ${ctx.runId}`);
    await ctx.click({x: 640, y: 360});
    await ctx.log("Clicked screen coordinate (640, 360)");
}
