import type {NyraContext} from "../runtime/context";

export default async function reaction(ctx: NyraContext) {
    await ctx.log(`Reaction start: ${ctx.runId}`);
    // This is a regular JS timer; the Nyra wait SDK is not implemented yet.
    await new Promise<void>((resolve) => setTimeout(resolve, 500));
    await ctx.log("Reaction asynchronous execution completed (log output by Rust host)");
}
