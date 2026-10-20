# Nyra

Nyra 是一款以增强用户感知、提供应对方案为长期目标的桌面辅助程序，使用 Rust + Bevy 分阶段实现。

当前一阶段提供 **配置驱动的短周期提醒浮窗**：通过置顶的单行提示、倒计时边框和颜色切换，持续提醒用户关注预先配置的事项。目前尚未实现屏幕感知、游戏状态判断或动态生成应对方案。

## 运行

当前面向 Windows，使用 DX12 渲染。已在 Rust 1.95.0 环境完成编译测试；锁定依赖中的 Bevy 版本为 0.19.1。

在项目目录运行：

```powershell
cargo run --locked
```

也可以指定配置文件：

```powershell
cargo run --locked -- "D:\Configs\nyra.json"
```

默认从 **进程工作目录**读取 `config.json`，并非自动从可执行文件所在目录读取。配置修改后需要重启。

浮窗内按住鼠标左键可拖动。当前没有锁定/穿透模式或应用内退出按钮；从终端启动时，可返回启动终端按 `Ctrl+C` 结束运行。

## 配置

```json
{
  "tips": [
    {
      "tip": "保持侦察",
      "interval": 8,
      "showTime": 5,
      "color": "#33E078",
      "reaction": {
        "hotkey": "Ctrl+Alt+R",
        "script": "scripts/respond.ts"
      }
    },
    {
      "tip": "持续生产",
      "interval": 6
    }
  ]
}
```

| 字段       | 含义                                                      |
|------------|-----------------------------------------------------------|
| `tips`     | 非空提示数组                                              |
| `tip`      | 非空提示文本，建议使用简短单行内容                        |
| `interval` | 展示结束后重新获得调度资格的间隔，正整数秒                |
| `showTime` | 可选，本轮展示时长，正整数秒，默认 5                      |
| `color`    | 可选，`#RRGGBB` 主题色；省略时按配置位置分配默认色板颜色  |
| `reaction` | 可选应对方案，包含全局快捷键 `hotkey` 与 TS 入口 `script` |

所有提示初次加载时均具备资格。每轮结束后，优先从其他已具备资格的提示中选择 **最早获得资格**的一项；资格时间相同时，依次按较短
`interval` 和配置顺序选择。若其他项都不具备资格，则立即重复当前项，以保持持续显示。

因此 `interval` 不保证固定出现频率，也不禁止连续重复。仅有一个提示时，该提示会持续按 `showTime` 开启新一轮。

边框剩余部分使用当前项颜色，已消耗部分使用预计下一项颜色。长文本会按估算宽度截断并追加 `...`。

## Bun 脚本运行器

Reaction 已接入独立 Bun 子进程运行器、tip 全局快捷键及宿主日志 SDK。正常模式在配置至少一个脚本时启动 Bun，预加载脚本并保持进程到
Nyra 退出；只有当前展示 tip 的快捷键会被注册。鼠标动作与宿主等待 SDK 尚未接入。

安装包携带 **Bun 1.3.10 Windows x64 baseline**，运行时位于 `nyra.exe` 旁的 `runtime/bun.exe`，不使用 PATH 中的 Bun，也不要求安装
Node.js 或 npm 包。 版本与官方压缩包校验值固定在 [runtime/bun.json](runtime/bun.json)
，来源为 [Bun 官方发布](https://github.com/oven-sh/bun/releases/tag/bun-v1.3.10)
及该版本的 [SHA256 清单](https://github.com/oven-sh/bun/releases/download/bun-v1.3.10/SHASUMS256.txt)。

开发者先构建，再生成包含 Bun 的可直接解压运行的发布包：

```powershell
cargo build --release --locked
./scripts/package.ps1
```

打包阶段下载并校验 Bun，输出 `target/distribution/Nyra.zip`。当前不提供用户安装脚本；解压后可直接运行，正式安装流程将在需要发布时重新设计。

`--run-script` 是不启动 Bevy 浮窗的一次性开发调试入口：

```powershell
./nyra.exe ./config.json --run-script ./scripts/macro.ts
```

`.ts` / `.mts` 脚本默认导出普通同步函数，例如保存为 `scripts/macro.ts`：

```typescript
import type {NyraContext} from "../runtime/sdk";

export default function macro(ctx: NyraContext) {
    ctx.log(`开始运行 ${ctx.runId}`);
    ctx.delay(1000);
    ctx.log("等待完成");
}
```

正常使用时，在 tip 中配置 `reaction.hotkey` 和 `reaction.script`。快捷键支持 `Ctrl`、`Alt`、`Shift`、`Win` 修饰键与字母、数字、
`F1..F24`、`Space`、`Enter`、`Escape`、`Tab`；功能键可单独使用，其他按键至少需要一个修饰键。脚本相对路径及 Bun
工作目录均按配置文件所在目录解析。Bun 在 Nyra 启动时自动转换并预加载 TS，运行时不检查类型；修改脚本后重启 Nyra 生效。
`ctx` 提供只读 `runId`、同步的 `log(message)`、`click(point, options?)`、`drag(from, to, options?)`、`delay(durationMs)`，以及
`click_in_grid`、`click_in_line`、`click_in_array` 点击辅助方法。每次调用立即向 Rust 发出请求，阻塞当前脚本线程直至操作完成；
每次点击或拖拽都按「事前条件 → 行为操作 → 事后动作」执行，三个阶段完成后调用才返回。
默认没有事前条件，事后等待 50 ms；宿主操作错误直接在调用处抛出，可使用普通 `try/catch` 处理。
日志附带运行标识和脚本行号，单条消息长度最多为 2000 个 UTF-16 码元。普通 `console` 输出作为诊断转发到 Rust stderr，Bun stdout
保留给生命周期与 SDK 请求协议。

通过 `set_defaults` 设置本轮后续操作的默认阶段，单次参数覆盖整个对应阶段，且不会修改默认值；省略字段继承默认值，`null` 显式关闭该阶段：

```typescript
ctx.set_defaults({
    before: {kind: "delay", durationMs: 100},
    after: {kind: "delay", durationMs: 50},
});
ctx.click({x: 640, y: 360});
ctx.click({x: 840, y: 360}, {
    before: {kind: "image", path: "conditions/menu.json", timeoutMs: 3000},
    after: null,
});
ctx.drag({x: 640, y: 360}, {x: 840, y: 360}, {before: null, durationMs: 300});
```

事前条件支持固定等待和已有 Perception JSON 图片条件；图片条件默认超时 3000 ms、轮询间隔 50 ms，路径相对主配置目录，
匹配阈值及模板路径取自条件文件。未匹配或目标失焦时继续等待；捕获失败、资源错误或超时会阻止本次输入。
事后动作当前支持固定等待，仅在输入成功后执行。`set_defaults` 同样作用于点击辅助方法和拖拽，每次宏调用重新创建默认值。
旧 `delayMs` 请迁移为 `after: {kind: "delay", durationMs: ...}`；旧占位 `condition` 参数改由 `before` 表达。

公共 SDK 的 `createContext(backend)` 接受实现 `ReactionBackend` 的后端；原生后端将完整 `ActionStep` 交给 Rust。
调试页面可复用同一脚本，并用 `createDebugContext({mark})` 接收从 1 开始编号的 `{sequence, step}`，由页面在点击位置绘制序号或绘制拖拽路径。
调试后端保留完整的事前、行为、事后配置供展示，不实际等待、截图或发送系统输入；页面负责将物理屏幕坐标换算为自身的显示坐标。

宏在单个 Bun 线程内同步执行，不需要 `async/await`；`async` 入口及返回 Promise 的入口会被拒绝。
运行期间不调度该线程的定时器或异步回调，等待操作请使用 `ctx.delay`；入口返回即结束本轮。
已有普通同步脚本可直接使用，旧异步脚本需要移除 `async` 与 SDK 调用前的 `await`，并将异步等待改为同步 SDK 调用。

SDK 绑定由公共 RPC 层处理。`runner.ts` 将后端调用统一编码为 `{ method, args }`。Rust 的
`src/reaction/sdk.rs` 是 SDK 结构体、方法签名、文档和宿主行为的唯一声明源；`define_sdk!` 生成方法匹配和位置参数反序列化，
`build.rs` 在任意 Cargo 构建时生成 `runtime/context.d.ts`。新增方法或结构体后运行 `cargo check` 即可刷新声明文件，不要手工编辑
`.d.ts`；新增原生方法时同时更新运行器的类型化后端绑定。用户侧的参数默认值和坐标辅助方法位于 `runtime/sdk.ts`。

根配置可增加 `"reaction": { "timeoutMs": 30000 }`，省略时使用 30 秒；允许范围为 1..=300000 毫秒。旧提示配置保持有效，没有脚本时不会启动
Bun。 Windows 会话中同时只允许一项 Nyra 宏，忙时拒绝新触发。单轮入口结束后 Bun
回到空闲并等待下一次触发。脚本异常只结束本轮，超时、取消、协议故障及宿主退出会回收托管进程。
Rust 独立管理取消及整轮超时；`delay` 按最多 10 ms 的等待分段检查，不会等完整延迟结束才处理取消（不保证硬实时响应）。
脚本输出单行最多 16 KiB、每次最多 1 MiB，超限会失败。脚本拥有 Bun 的本机能力，此运行器用于可信用户脚本。

仅开发运行器时可用 `cargo build --locked`，再执行
`./scripts/package.ps1 -Executable ./target/debug/nyra.exe -OutputDirectory ./target/distribution/Nyra-dev`。

在 RustRover 中开发时，先运行 `./scripts/prepare-dev-env.ps1`。该脚本调用 `install-bun-runtime.ps1 -RuntimeDirectory <路径>`
，将固定版本 Bun 和引导文件准备到 `target/debug/runtime`，并按锁文件安装 TypeScript 开发依赖，再选择共享运行配置
**Reaction**。 准备脚本优先复用本地已校验的下载缓存，不安装系统级 Bun，也不生成安装包。首次使用、`cargo clean` 后或修改
`runtime/` 中的文件后重新执行；修改 [examples/reaction.ts](examples/reaction.ts) 后直接重跑配置即可。 该配置启动 Nyra
并执行示例宏，不打开浮窗；`runner.ts` 由 Nyra 完成握手后加载，不应直接作为用户宏入口。

运行器已通过 Rust 编译检查与开发构建，TS 引导层通过类型检查；正常模式已验证全局快捷键触发两轮脚本时复用同一 Bun 进程，
`--run-script` 仍能独立运行示例宏。并发拒绝、超时与正常退出清理等行为验收仍待完成，尚无通信延迟基准数据。

## 当前边界

- 窗口固定为 160 × 40，尺寸、字体及动画参数暂未开放配置。
- 兼容目标为桌面、窗口化及无边框全屏；具体游戏和显示环境仍需实测，独占全屏不属于当前兼容承诺。
- 未实现鼠标穿透，浮窗可能截获其覆盖区域的鼠标操作。
- 未实现配置热加载、位置持久化、暂停/恢复和全局快捷键。
- 字体与 DPI 可能影响文本宽度及裁切效果。
- 尚未配置专门的低功耗刷新策略，资源占用待测量。

## 文档

- [开发路线图](ROADMAP.md)：短期需求及 Sensory、Detection、Reaction 的阶段目标。
- [一阶段实现总结](docs/phase1-summary.md)：功能、完整使用方法、调度示例、架构、验证记录与优化方向。
