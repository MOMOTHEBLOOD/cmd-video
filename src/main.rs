use std::path::PathBuf;

use anyhow::{bail, Result};
use clap::Parser;

/// 在 CMD 终端里播放视频（半块真彩 ANSI · 静音）
#[derive(Parser)]
#[command(name = "cmd-video", version)]
struct Cli {
    /// 视频文件路径
    video: PathBuf,
    /// 帧率上限（1~60）
    #[arg(long, default_value_t = 20.0)]
    fps_cap: f64,
    /// ffmpeg / ffprobe 所在目录
    #[arg(long)]
    ffmpeg: Option<PathBuf>,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    if !cli.video.is_file() {
        bail!("文件不存在：{}", cli.video.display());
    }
    cmd_video::app::run(
        &cli.video,
        &cmd_video::app::Config {
            fps_cap: cli.fps_cap.clamp(1.0, 60.0),
            ffmpeg_dir: cli.ffmpeg,
        },
    )
}
