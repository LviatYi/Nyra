# Nyra

Nyra 是一个以增强桌面程序用户感知为长期目标的 Rust 项目。

技术栈 Rust + Bevy

## Phase1

实现一个运行于 Windows 桌面的 **短周期 TODO 轮转提示 Overlay**。

窗口描述：

- 无头圆角小窗口，大小仅占用 160*60px。
- 始终置顶显示，可覆盖在窗口化、 Borderless Fullscreen 或全屏游戏上。
- 可拖拽。

窗口内容描述：

- 单行文本。超出长度的内容显示为 `...`，表达 Top 内容。
- 底色。
- 外部边框，边框线条具有功能，可沿着边缘按照进度变色，表达该项 Tip 的剩余展示时间。

数据层：

- 通过 json 进行配置。
- 字段包含：
    - **tips** 数组
        - **tipString** tip 内容
        - **interval** 展示间隔，根据间隔与配置顺序决定 tip 展示顺序
        - **showTime** (optional) 展示时间

所有时间单位为秒。

## 运行

需要 Rust 1.95 或更高版本。在项目目录运行：

```text
cargo run
```

默认读取项目目录中的 `config.json`。也可以传入其他配置文件：

```text
cargo run -- path/to/config.json
```

浮窗可在任意位置按住鼠标左键拖动。

## 配置语义

`tips` 按数组顺序循环。每项出现后，经过该项的 `interval` 切换到下一项；
`showTime` 表示本轮实际可见时长，省略时等于 `interval`。如果指定
`showTime`，其值必须不大于 `interval`，剩余间隔内浮窗会透明隐藏。

进度边框表示当前项 `showTime` 的剩余比例。文本过长时会截断并显示 `...`。

## 当前限制

- Always-on-top 可覆盖桌面窗口、窗口化游戏和 Borderless Fullscreen 游戏，但无法保证覆盖采用独占全屏模式的游戏。
- 配置在启动时读取，修改后需要重启程序。
- 文本截断按 Unicode 显示宽度估算，以保持实现简单；不同系统字体下的实际宽度可能略有差异。
