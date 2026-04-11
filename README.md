# Nyra

一个用于 MVP 实验的最小 OCR 原型：截取屏幕上的指定矩形区域，使用 Tesseract 识别文本，并将结果输出到终端。

当前版本只覆盖最小闭环：

- 指定屏幕全局坐标区域
- 截图该区域
- 使用 Tesseract OCR
- 判断结果是否包含目标文本
- 输出 `success` 或 `failed. original text: ...`

## 依赖模型

项目使用 `tesseract-rs`，但为了避免 Windows 上调试版 Leptonica 的额外日志，仓库约定单独构建一套固定的 OCR `Release` 依赖：

- Leptonica `1.84.1`
- Tesseract `5.3.4`
- 默认语言数据：`eng`、`chi`、`chi_sim`

这些产物会被放到仓库本地的 `.ocr-release/`，并同步到 `%APPDATA%\tesseract-rs\...`，这样无论 `cargo run` 还是 `cargo run --release`，都会复用同一套 `Release` OCR 库。

## Windows 开发环境

需要在 Windows 下准备：

1. Visual Studio 2022 Build Tools 或 Visual Studio 2022
2. CMake
3. 一个带 MSVC 环境的 PowerShell

推荐直接打开 `Developer PowerShell for VS 2022` 再执行后续命令。

## 初始化

首次拉取仓库后，先执行：

```powershell
.\scripts\setup-dev.ps1
```

这个脚本会自动：

- 下载固定版本的 Leptonica/Tesseract 源码
- 打掉 Leptonica 中会输出 `Pix colormap...` 的调试代码
- 构建 `Release` 版 OCR 库
- 安装到仓库的 `.ocr-release/`
- 同步到 `%APPDATA%\tesseract-rs\...`
- 下载默认 `tessdata`

如果你想强制重建 OCR 依赖：

```powershell
.\scripts\setup-dev.ps1 -ForceRebuild
```

## 用法

运行格式：

```powershell
cargo run --package nyra --bin nyra -- <x1> <y1> <x2> <y2> <text>
```

例如：

```powershell
cargo run --package nyra --bin nyra -- 250 0 350 50 nyra
```

识别结果包含目标文本时输出：

```text
success
```

否则输出：

```text
failed. original text: <recognized text>
```

## 分发

如果后续要打包分发，建议至少带上：

- `nyra.exe`
- `tessdata/`

更稳的做法是让分发包启动时显式设置 `TESSDATA_PREFIX` 指向程序目录下的 `tessdata`。

## 仓库内目录

- `.build/`: CMake 构建目录和中间产物
- `.ocr-release/`: 安装后的固定 `Release` OCR 库
- `scripts/setup-dev.ps1`: 幂等开发机初始化入口
- `scripts/build-ocr-release.ps1`: OCR `Release` 依赖构建脚本
