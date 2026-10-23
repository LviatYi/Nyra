import type { NyraContext } from "../runtime/sdk";

export default function imageTestPanelReady(ctx: NyraContext) {
    // Replace the 999 placeholders before running; region dimensions must match the template.
    ctx.click({x: 999, y: 999}, {
        before: {
            kind: "image",
            target: {
                mode: "window",
                title: "999",
                region: {x: 999, y: 999, width: 999, height: 999},
            },
            template: "conditions/panel-ready.png",
            threshold: 0.95,
            timeoutMs: 999,
            pollIntervalMs: 999,
        },
    });
}
