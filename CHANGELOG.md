# Changelog

格式基于 [Keep a Changelog](https://keepachangelog.com/zh-CN/1.1.0/)，
版本遵循 [SemVer](https://semver.org/lang/zh-CN/)。

## [0.1.0] - 2026-10-04

### Added

- 终端视频播放：ffmpeg rawvideo 管道解码 + 半块（▀）真彩 ANSI 渲染
- 元数据探测（ffprobe JSON：分辨率 / 帧率 / 时长），帧率上限可配（默认 20fps）
- 帧时钟：墙钟推进、管道背压、落后自动追帧
- 播放控制：Space 暂停/继续/重播，←/→ 快退快进 ±10s（-ss 快速定位）
- 终端尺寸自适应（缩放交给 ffmpeg scale），状态栏（状态/文件/时间/帧率）
- ffmpeg 工具查找：--ffmpeg / CMD_VIDEO_FFMPEG / vendor 目录 / PATH
- E2E：winpty 真机捕获验证帧流、暂停、快进、结束与状态栏
