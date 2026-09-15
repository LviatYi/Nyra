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

`.ts` / `.mts` 脚本默认导出返回 Promise 的函数，例如保存为 `scripts/macro.ts`：

```typescript
import type {NyraContext} from "../runtime/context";

export default async function macro(ctx: NyraContext) {
    await ctx.log(`开始运行 ${ctx.runId}`);
}
```

正常使用时，在 tip 中配置 `reaction.hotkey` 和 `reaction.script`。快捷键支持 `Ctrl`、`Alt`、`Shift`、`Win` 修饰键与字母、数字、
`F1..F24`、`Space`、`Enter`、`Escape`、`Tab`；功能键可单独使用，其他按键至少需要一个修饰键。脚本相对路径及 Bun
工作目录均按配置文件所在目录解析。Bun 在 Nyra 启动时自动转换并预加载 TS，运行时不检查类型；修改脚本后重启 Nyra 生效。
`ctx` 提供只读 `runId` 和 `log(message): Promise<void>`。`await ctx.log(...)` 通过协议请求 Rust 写入自身
stdout，收到宿主确认后继续执行；日志附带运行标识，单条消息长度最多为 2000 个 UTF-16 码元。 SDK 调用需逐个 `await`
，并行调用会失败，入口返回时仍有未完成 SDK 调用也会失败。普通 `console` 输出继续作为诊断转发到 Rust stderr，Bun stdout
保留给生命周期与 SDK 请求协议。

SDK 绑定由公共 RPC 层处理。`runner.ts` 通过动态上下文将任意方法调用统一编码为 `{ method, args }`，Rust 的
`define_sdk_bindings!` 宏负责方法匹配和位置参数反序列化。新增一个返回 `Promise<void>` 的宿主方法时，只需在
`runtime/context.d.ts` 增加面向宏作者的类型声明，并在 `define_sdk_bindings!` 中增加包含宿主行为的绑定条目；不再单独编写 TS
包装函数、请求/响应关联或 Rust 参数解析代码。

根配置可增加 `"reaction": { "timeoutMs": 30000 }`，省略时使用 30 秒；允许范围为 1..=300000 毫秒。旧提示配置保持有效，没有脚本时不会启动
Bun。 Windows 会话中同时只允许一项 Nyra 宏，忙时拒绝新触发。单轮入口结束后 Bun
回到空闲并等待下一次触发；未等待的后台工作不保证完成。脚本异常只结束本轮，超时、取消、协议故障及宿主退出会回收托管进程。
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
