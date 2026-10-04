#![forbid(unsafe_code)]

//! cmd-video：在 CMD 终端里播放视频（半块真彩 ANSI + ffmpeg 管道，静音）。
//!
//! 分层约定：
//! - `ff.rs`    ffmpeg/ffprobe 进程封装（探测 + rawvideo 解码管道）
//! - `render.rs` 帧渲染纯函数（RGB24 → 半块 ANSI）
//! - `player.rs` 播放时钟与状态（纯逻辑）
//! - `app.rs`   终端装配与事件循环

pub mod app;
pub mod ff;
pub mod player;
pub mod render;
