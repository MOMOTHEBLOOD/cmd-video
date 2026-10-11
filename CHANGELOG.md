# Changelog

格式基于 [Keep a Changelog](https://keepachangelog.com/zh-CN/1.1.0/)，
版本遵循 [SemVer](https://semver.org/lang/zh-CN/)。

## [0.3.0] - 2026-10-11

### Added

- **GitHub Actions CI**：与 cmd-reader / cmd-share 对齐的五道门
  （`cargo fmt --check` → `clippy --all-targets -D warnings` → `test`
  → `build --release` → 上传 exe artifact），windows-latest
- README 补终端实拍截图与性能实测表（见「实测」小节）

### Changed

- 版本号与 cmd-reader / cmd-share 统一推进到 0.3.0（三件套同节奏发布）

## [0.2.0] - 2026-10-04

### Added

- **ASCII 字符画模式**（`--ascii`）：亮度（BT.601）映射到字符梯度
  `" .':^-~=+*!?#%&$@"`，单色播放
- `--invert` 反转亮度梯度（浅色主题终端用）

### Fixed

- **纵横比修正**：此前视频被拉伸到整个终端区域，竖屏视频严重变形；
  现在按"竖向像素 = 2 × 横向像素"计算字符格数，竖屏/横屏均保持原始比例
  （解码尺寸改为按像素传入，渲染模式只决定每格像素数）

## [0.1.0] - 2026-10-04

### Added

- 终端视频播放：ffmpeg rawvideo 管道解码 + 半块（▀）真彩 ANSI 渲染
- 元数据探测（ffprobe JSON：分辨率 / 帧率 / 时长），帧率上限可配（默认 20fps）
- 帧时钟：墙钟推进、管道背压、落后自动追帧
- 播放控制：Space 暂停/继续/重播，←/→ 快退快进 ±10s（-ss 快速定位）
- 终端尺寸自适应（缩放交给 ffmpeg scale），状态栏（状态/文件/时间/帧率）
- ffmpeg 工具查找：--ffmpeg / CMD_VIDEO_FFMPEG / vendor 目录 / PATH
- E2E：winpty 真机捕获验证帧流、暂停、快进、结束与状态栏
