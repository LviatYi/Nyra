# Nyra Phase 1：RTS 阶段提示浮窗

## 1. 项目背景

Nyra 是一个以增强桌面程序用户感知为长期目标的 Rust 项目，主要应用场景为游戏，尤其是 RTS 游戏。

项目此前的开发存在方向性问题：

* 过早关注整体架构设计；
* 为尚未实际出现的需求设计大量抽象；
* 优先尝试 OCR、屏幕感知等高复杂度能力；
* 导致很长时间内无法形成可实际使用的产品能力。

本轮重新开始开发。

**Phase 1 的首要原则是尽快形成一个可以真实使用、真实验证的最小功能。**

不要为未来需求提前建设复杂架构。

---

# 2. Phase 1 产品目标

实现一个运行于 Windows 桌面的 **RTS 阶段提示 Overlay**。

程序显示一个：

* 无系统标题栏；
* 可配置尺寸；
* 可拖动；
* 始终置顶；
* 可覆盖在窗口化或 Borderless Fullscreen 游戏上；
* 可切换鼠标交互/穿透状态；

的浮动窗口。

浮窗按照配置好的阶段顺序，轮转显示当前阶段对应的提示信息。

它的用途不是分析游戏，而是：

> 在玩家游玩 RTS 时，持续提示玩家“当前阶段应该重点关注什么”。

例如：

```text
当前阶段：早期扩张

• 不要浮余资源
• 持续生产单位
• 保持侦察
```

Phase 1 **不自动判断游戏当前处于哪个阶段**。

阶段推进暂时仅由程序自身的简单状态控制完成。

---

# 3. 技术栈

优先使用：

```text
Rust
Bevy 0.19.x
Bevy UI
bevy_winit / winit
serde
TOML
```

窗口能力优先使用 Bevy 已提供的 API。

除非实际验证确认 Bevy/winit 无法满足某项必要能力，否则：

**不要直接引入 Win32 API。**

不要因为未来可能需要平台相关能力而提前创建复杂的平台抽象层。

---

# 4. 核心功能

## 4.1 Overlay 窗口

创建一个独立的 Bevy 窗口，至少满足：

* 无边框；
* 无系统窗口装饰；
* 支持透明背景；
* 始终置顶；
* Windows 下不显示在任务栏；
* 初始尺寸可以通过配置指定；
* 用户可以拖动窗口改变位置。

优先使用 Bevy：

```rust
Window {
decorations: false,
transparent: true,
window_level: WindowLevel::AlwaysOnTop,
skip_taskbar: true,
..
}
```

以及 Bevy 提供的窗口拖拽 API：

```rust
Window::start_drag_move()
```

不要自行实现窗口坐标 delta 拖拽，除非 Bevy API 被实际证明不可用。

---

# 5. Overlay 交互模式

Overlay 至少存在两种模式：

```rust
enum OverlayMode {
    Editing,
    Locked,
}
```

## Editing

用于调整浮窗。

此状态：

* 窗口接受鼠标输入；
* 可以通过鼠标拖动窗口；
* 不要求鼠标穿透。

## Locked

用于实际游戏。

此状态：

* Overlay 不应阻挡游戏鼠标操作；
* 开启窗口 hit-test / mouse passthrough；
* 不允许拖动。

模式之间需要存在一个简单、可靠的切换方式。

Phase 1 可以使用键盘快捷键。

具体快捷键可以选择一个实现方便且不容易误触的组合，例如：

```text
Ctrl + Shift + N
```

快捷键是否能够在游戏获得焦点时工作，如果 Bevy 普通键盘输入无法做到，不要立刻引入复杂实现。

首先记录这一限制。

全局快捷键可以作为后续小范围扩展项单独处理。

---

# 6. 阶段数据

阶段信息必须与程序代码分离。

使用 TOML 配置。

例如：

```toml
[[stages]]
title = "开局"
duration = 60

lines = [
    "持续生产单位",
    "避免资源浮余",
    "进行第一次侦察"
]

[[stages]]
title = "早期扩张"
duration = 120

lines = [
    "建立第二经济点",
    "确认敌人科技路线",
    "保持生产设施运转"
]
```

字段至少包括：

```text
title
duration
lines
```

其中：

* `title`：阶段名称；
* `duration`：该阶段持续时间，单位秒；
* `lines`：当前阶段需要提示玩家关注的信息。

允许根据实现需要小幅修改配置格式，但不要引入复杂 schema。

---

# 7. 阶段轮转

程序启动后：

```text
Stage 0
   ↓ duration
Stage 1
   ↓ duration
Stage 2
   ↓
...
```

当前阶段结束后自动进入下一阶段。

到达最后一个阶段后，可以：

* 保持最后阶段；

或：

* 根据一个简单配置决定是否循环。

Phase 1 不要求自动识别游戏状态。

不要实现：

* OCR；
* CV；
* 屏幕截图分析；
* 游戏内存读取；
* 游戏 API；
* AI 判断阶段。

---

# 8. UI

Phase 1 使用最基本的 Bevy UI 即可。

允许使用：

```text
Node
Text
BackgroundColor
Border
```

UI 至少显示：

```text
阶段标题

提示内容 1
提示内容 2
提示内容 3
...
```

视觉要求只有：

* 在游戏画面上具有足够可读性；
* 窗口背景可以透明或半透明；
* 文本清晰；
* 布局不发生明显溢出。

不要在 Phase 1 建立完整设计系统。

不要优先引入：

* BSN；
* Feathers；
* 自定义 UI framework；
* theme framework；
* widget framework；
* animation framework。

如果实现阶段切换的简单淡入淡出非常容易，可以加入；否则不属于 Phase 1 必需项。

---

# 9. 运行模型

Nyra 是桌面 Overlay，而不是一个需要持续高帧率运行的游戏。

因此优先采用：

```rust
WinitSettings::desktop_app()
```

或当前 Bevy 版本中对应的响应式/低功耗更新模式。

目标是：

> UI 没有变化时，不应为了显示静态文本持续进行高帧率更新。

阶段计时仍然需要正常工作。

如果 Bevy 的响应式运行模式与 Timer 更新存在冲突，应采用最简单可靠的方法定期唤醒应用，而不是退回长期 60/144 FPS 空转。

---

# 10. Phase 1 明确不做的事情

以下全部属于 **Non-goals**。

不要在本阶段实现：

### 游戏感知

* OCR；
* CV；
* 截图识别；
* 游戏状态识别；
* 单位识别；
* UI 元素识别。

### 高级 Overlay

* DirectX/Vulkan hooking；
* DLL injection；
* Fullscreen Exclusive Overlay；
* 游戏进程注入。

Phase 1 只要求：

```text
Desktop
Windowed Game
Borderless Fullscreen
```

真正的 Fullscreen Exclusive 不属于兼容目标。

### 复杂配置系统

不要实现：

* GUI 设置页面；
* 配置编辑器；
* profile 管理器；
* 云同步。

直接编辑 TOML 即可。

### 插件系统

不要实现：

* Plugin SDK；
* scripting；
* Lua；
* WASM；
* 动态插件；
* event bus。

### 复杂架构

除非当前代码已经达到明显无法维护的程度，否则不要创建类似：

```text
nyra-core
nyra-runtime
nyra-platform
nyra-overlay
nyra-ui
nyra-domain
nyra-service
nyra-plugin-api
```

的 crate 分层。

Phase 1 优先保持单 crate。

---

# 11. 推荐代码规模与结构

目标不是严格限制行数，而是避免架构膨胀。

推荐初始结构：

```text
nyra/
├── Cargo.toml
├── config.toml
├── assets/
│   └── fonts/
└── src/
    ├── main.rs
    ├── config.rs
    └── overlay.rs
```

如果实现足够简单，甚至允许：

```text
src/
├── main.rs
└── config.rs
```

不要为了“代码整洁”提前拆出大量模块。

---

# 12. 推荐领域模型

只建立当前需求真正需要的少量数据类型。

例如：

```rust
struct Stage {
    title: String,
    duration: Duration,
    lines: Vec<String>,
}
```

运行状态可以类似：

```rust
struct StageState {
    current: usize,
    timer: Timer,
}
```

以及：

```rust
enum OverlayMode {
    Editing,
    Locked,
}
```

这些类型不要求严格按照示例实现。

原则是：

> 数据模型服务于当前功能，而不是预测未来 Nyra 的完整领域模型。

---

# 13. 推荐系统

预计只需要类似：

```text
setup_overlay
rotate_stage
update_stage_ui
toggle_overlay_mode
drag_overlay
```

如果需要额外少量 system 可以增加。

不要为了这些 system 再构造通用 command bus、controller/service 层等结构。

---

# 14. 错误处理

以下错误需要能够明确输出：

* `config.toml` 不存在；
* TOML 无法解析；
* stages 为空；
* duration 非法；
* 字体或必要 asset 无法加载。

开发阶段可以使用：

```text
tracing / log
```

输出诊断信息。

不要求设计面向最终用户的完整错误 UI。

---

# 15. 验收标准

Phase 1 完成必须能够人工执行以下测试。

## Test 1 — 启动

运行：

```text
cargo run
```

程序成功启动并读取 `config.toml`。

出现 Overlay。

---

## Test 2 — 窗口

Overlay：

* 无标题栏；
* 无边框；
* 位于普通窗口上方；
* 可以显示在 Borderless Fullscreen 游戏之上；
* Windows 任务栏中不出现普通应用窗口按钮。

---

## Test 3 — 拖动

进入：

```text
Editing
```

后，可以使用鼠标拖动 Overlay。

移动行为正常，不出现明显跳跃。

---

## Test 4 — 游戏输入

进入：

```text
Locked
```

后：

鼠标点击 Overlay 所覆盖的位置时，应作用于其下方的游戏/桌面窗口。

Nyra 不应阻挡点击。

---

## Test 5 — 模式切换

可以可靠地：

```text
Editing
⇄
Locked
```

切换。

---

## Test 6 — 阶段轮转

给定测试配置：

```text
A — 5 秒
B — 5 秒
C — 5 秒
```

程序能够按照：

```text
A → B → C
```

正确更新 UI。

---

## Test 7 — 配置驱动

修改：

```text
title
duration
lines
```

后重新启动程序，无需修改 Rust 源码即可得到对应变化。

---

## Test 8 — 资源占用

当 Overlay 静止且没有动画时：

程序不应因为默认游戏循环而长期以高 FPS 无意义刷新。

无需对性能进行深度优化，但必须确认没有明显的持续高 CPU/GPU 占用。

---

# 16. 开发原则

整个 Phase 1 遵守以下优先级：

```text
可运行
>
可验证
>
简单
>
可维护
>
为未来扩展
```

遇到设计选择时：

如果方案 A：

```text
20 行代码
只能解决当前需求
```

而方案 B：

```text
200 行代码
建立一个以后可能有用的抽象
```

默认选择 A。

只有当前需求已经证明需要抽象时才建立抽象。

---

# 17. 本阶段最终交付物

完成 Phase 1 后应至少得到：

1. 一个可以直接运行的 Nyra Rust/Bevy 程序；
2. 一个示例 `config.toml`；
3. 可拖拽透明 Always-On-Top Overlay；
4. `Editing / Locked` 两种模式；
5. Locked 状态鼠标穿透；
6. 按配置自动轮转的阶段提示；
7. 简短 README，包含：

    * 如何运行；
    * 如何修改阶段配置；
    * 如何切换 Editing/Locked；
    * 当前已知限制。

**完成上述内容即停止 Phase 1。**

不要在同一个任务中继续实现 OCR、自动游戏状态识别、复杂配置 UI 或插件系统。
