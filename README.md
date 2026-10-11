# cmd-video

在 CMD 终端里播放视频。Rust + crossterm 实现，`▀`（上半块）字符一格画两个像素、
24 位真彩 ANSI 高速刷屏，ffmpeg 管道解码。产物为单个 `cmd-video.exe`（约 1.1MB）。

> **预期管理**：这是"演示级"观影——分辨率 ≈ 终端格子数（全屏约 200×100 像素）、
> 帧率受终端吞吐限制（Windows Terminal 实测 12~20fps）、无声音。
> 它赢在"居然能在终端里放视频"，不赢在画质。

## 构建

```cmd
cargo build --release
target\release\cmd-video.exe 视频.mp4
```

## ffmpeg 依赖（运行时）

解码由 ffmpeg 完成，查找顺序：`--ffmpeg 目录` > 环境变量 `CMD_VIDEO_FFMPEG` >
可执行文件同级 `ffmpeg\bin` > 项目 `vendor\ffmpeg\bin` > PATH。

任选其一：

1. 把 ffmpeg 静态构建解压到 `vendor\ffmpeg`（使其含 `bin\ffmpeg.exe` 与 `bin\ffprobe.exe`）
2. 安装 ffmpeg 后加入 PATH
3. 运行时传 `--ffmpeg <目录>`

## 快捷键

| 按键 | 作用 |
|---|---|
| `Space` | 暂停 / 继续（播完时 = 重播） |
| `←` / `→` | 快退 / 快进 10 秒 |
| `q` / `Esc` / `Ctrl+C` | 退出 |

状态栏：播放状态 · 文件名 · 进度时间 / 总时长 · 帧率。

## 两种画质模式

```cmd
cmd-video 视频.mp4            :: 默认：半块真彩（▀ + 24位色）
cmd-video --ascii 视频.mp4    :: ASCII 字符画（单色，亮度→字符梯度）
cmd-video --ascii --invert 视频.mp4  :: 浅色主题终端反转梯度
```

- 半块真彩：`▀`（上半块）一格两个像素（前景=上、背景=下），彩色
- ASCII：一像素一字符，梯度 `" .':^-~=+*!?#%&$@"`（暗→亮）
- 两种模式都按视频原始纵横比计算字符格数（终端格子高≈宽两倍）

## 架构

```
src/
├── ff.rs      ffmpeg/ffprobe 进程封装：ffprobe JSON 探测 + rawvideo 解码管道
├── render.rs  帧渲染纯函数：RGB24 → 半块真彩 ANSI（零 IO，可单测）
├── player.rs  播放时钟：墙钟 + 帧号推进，暂停/跳转只改锚点（纯逻辑，可单测）
└── app.rs     终端装配、事件循环、状态栏
```

设计要点：

- **管道背压**：ffmpeg 解码快于渲染时被管道自动阻塞，播放节奏完全由帧时钟控制，
  落后于墙钟时自动追帧（不睡眠直接读下一帧）
- **快进/快退** = 按目标时间戳重启 ffmpeg（`-ss` 快速定位），时钟重新锚定
- **尺寸自适应**：视频经 ffmpeg `scale` 直接缩放到终端格子数（垂直 ×2 像素），
  改变窗口大小即时生效
- **不重复造轮子**：不引入 ratatui（视频是纯刷屏场景），依赖仅 crossterm + clap

## 实测

**环境**：Windows 11 · Windows Terminal · 单 exe 约 1.1MB · 无声音

| 项 | 实测值 |
|---|---|
| 终端帧率 | 12~20fps（受终端刷屏吞吐限制，非解码瓶颈） |
| 分辨率 | ≈ 终端格子数，全屏约 200×100 像素 |
| 色彩 | 24 位真彩 ANSI（`▀` 半块，每格 2 像素） |
| 管道延迟 | 落后墙钟自动追帧，无累积漂移 |
| 体积 | 1.1MB（`lto=thin` + `strip=true`，零运行时依赖） |

> 帧率数字是**终端渲染上限**，不是代码性能瓶颈——视频源再高帧也只显示到终端吞吐为止。

## 已知边界

- 声音：终端无声道，MVP 静音（音频管道 + WASAPI 混音在 Roadmap）
- 老式 conhost：慢且 winpty 会把真彩降为 8 位色；推荐 Windows Terminal
- 无 seek 进度条交互，仅 ±10s 步进

## 路线图

- [ ] 音频：ffmpeg 音频管道 → WASAPI（rodio），墙钟同步
- [ ] Sixel 输出模式（Windows Terminal 1.22+ 真位图，画质一档）
- [ ] 播放列表 / 目录连播
- [ ] 姊妹项目：cmd-reader（终端电子书阅读器，共享 crossterm 底座）

## 开发

```cmd
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo build --release
```

五道门（fmt / clippy / test / release build / artifact 上传）由
`.github/workflows/ci.yml` 在 windows-latest 上自动执行，与
[cmd-reader](../cmd-reader)、[cmd-share](../cmd-share) 对齐。

## License

MIT
