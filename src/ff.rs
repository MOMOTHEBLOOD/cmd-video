//! ffmpeg / ffprobe 进程封装：元数据探测 + rawvideo 解码管道。
//!
//! 解码协议：`ffmpeg -f rawvideo -pix_fmt rgb24 -vf scale=WxH -` 向 stdout
//! 逐帧输出 W×H×3 字节。管道自带背压——ffmpeg 解码快于渲染时自动阻塞，
//! 因此播放节奏完全由本程序的帧时钟控制。
//!
//! 启动方式：程序名用字面量 `"ffmpeg"` / `"ffprobe"`，通过为子进程前置 PATH
//! 指向已解析的工具目录；文件路径等用户输入永远作为独立参数传递，
//! 全程不经 shell，不存在命令拼接。

use std::env;
use std::io::{BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStderr, ChildStdout, Command, Stdio};

use anyhow::{bail, Context, Result};

#[derive(Debug, Clone, Copy)]
pub struct VideoMeta {
    pub width: u32,
    pub height: u32,
    pub fps: f64,
    pub duration: Option<f64>,
}

/// 解析包含 ffmpeg.exe / ffprobe.exe 的工具目录。
/// 优先级：显式参数 > CMD_VIDEO_FFMPEG 目录 > 可执行文件同级 / 项目 vendor > PATH。
pub fn find_bin_dir(explicit: Option<&Path>) -> Result<PathBuf> {
    const TOOLS: [&str; 2] = ["ffmpeg.exe", "ffprobe.exe"];

    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Some(p) = explicit {
        candidates.push(p.to_owned());
    }
    if let Ok(dir) = env::var("CMD_VIDEO_FFMPEG") {
        candidates.push(PathBuf::from(dir));
    }
    if let Ok(exe) = env::current_exe() {
        if let Some(dir) = exe.parent() {
            candidates.push(dir.join("ffmpeg").join("bin"));
            // 开发环境：target/release → 项目根/vendor/ffmpeg/bin
            if let Some(root) = dir.parent().and_then(|d| d.parent()) {
                candidates.push(root.join("vendor").join("ffmpeg").join("bin"));
            }
        }
    }
    if let Ok(path_var) = env::var("PATH") {
        candidates.extend(env::split_paths(&path_var));
    }

    for dir in candidates {
        if TOOLS.iter().all(|t| dir.join(t).is_file()) {
            return Ok(dir);
        }
    }
    bail!("找不到 ffmpeg / ffprobe。可用 --ffmpeg 指定目录、设置 CMD_VIDEO_FFMPEG，或将其加入 PATH")
}

/// 让字面量程序名 "ffmpeg" 能从工具目录解析：前置子进程 PATH
fn with_tool_path<'a>(cmd: &'a mut Command, bin_dir: &Path) -> Result<&'a mut Command> {
    let child_path = env::join_paths(
        [bin_dir.to_path_buf()]
            .into_iter()
            .chain(env::split_paths(&env::var("PATH").unwrap_or_default())),
    )?;
    Ok(cmd.env("PATH", child_path))
}

/// 解析 ffprobe 的 "num/den" 帧率写法
pub fn parse_rate(s: &str) -> Option<f64> {
    let (num, den) = s.split_once('/')?;
    let num: f64 = num.trim().parse().ok()?;
    let den: f64 = den.trim().parse().ok()?;
    (den != 0.0).then(|| num / den)
}

pub fn probe(bin_dir: &Path, input: &Path) -> Result<VideoMeta> {
    let out = with_tool_path(&mut Command::new("ffprobe"), bin_dir)?
        .args([
            "-v",
            "error",
            "-select_streams",
            "v:0",
            "-show_entries",
            "stream=width,height,r_frame_rate",
            "-show_entries",
            "format=duration",
            "-of",
            "json",
        ])
        .arg(input)
        .output()
        .context("启动 ffprobe 失败")?;
    if !out.status.success() {
        bail!(
            "无法读取视频（ffprobe）：{}",
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    let json: serde_json::Value =
        serde_json::from_slice(&out.stdout).context("解析 ffprobe 输出失败")?;
    let stream = &json["streams"][0];
    let width = stream["width"].as_u64().context("缺少视频宽度")? as u32;
    let height = stream["height"].as_u64().context("缺少视频高度")? as u32;
    let fps = stream["r_frame_rate"]
        .as_str()
        .and_then(parse_rate)
        .context("缺少帧率")?;
    if width == 0 || height == 0 || !(fps.is_finite() && fps > 0.0) {
        bail!("无效的视频元数据：{width}x{height} @ {fps}");
    }
    Ok(VideoMeta {
        width,
        height,
        fps,
        duration: json["format"]["duration"]
            .as_str()
            .and_then(|s| s.parse().ok()),
    })
}

/// 构造缩放滤镜参数 "scale=W:H"。输入是经过钳位的终端尺寸数值（u32）。
fn scale_arg(width: u32, px_h: u32) -> String {
    let mut s = String::from("scale=");
    s.push_str(&width.to_string());
    s.push(':');
    s.push_str(&px_h.to_string());
    s
}

/// 构造快进参数 "-ss" 的取值（毫秒精度的小数秒字符串）。
/// 输入为内部播放时钟的数值位置；非法值返回 None（不跳转）。
fn seek_arg(secs: f64) -> Option<String> {
    if !secs.is_finite() || secs <= 0.0 {
        return None;
    }
    let ms = (secs * 1000.0).clamp(0.0, u64::MAX as f64) as u64;
    let whole = (ms / 1000).to_string();
    let frac = (ms % 1000).to_string();
    let mut s = whole;
    s.push('.');
    s.push_str(&"0".repeat(3usize.saturating_sub(frac.len())));
    s.push_str(&frac);
    Some(s)
}

/// ffmpeg rawvideo 解码器。Drop 时自动杀掉子进程。
pub struct Decoder {
    child: Child,
    stdout: BufReader<ChildStdout>,
    stderr: ChildStderr,
    first_frame: bool,
    pub frame_bytes: usize,
    pub px_w: u32,
    pub px_h: u32,
}

/// 启动解码管道：输出 px_w × px_h 像素的 RGB24 裸帧。
/// 像素尺寸由调用方按渲染模式与终端纵横比计算好传入。
pub fn spawn_decoder(
    bin_dir: &Path,
    input: &Path,
    seek: f64,
    px_w: u32,
    px_h: u32,
) -> Result<Decoder> {
    let mut base = Command::new("ffmpeg");
    let cmd = with_tool_path(&mut base, bin_dir)?;
    cmd.args(["-hide_banner", "-loglevel", "error", "-nostdin"]);
    if let Some(ss) = seek_arg(seek) {
        cmd.args(["-ss", &ss]);
    }
    cmd.arg("-i").arg(input).args([
        "-f",
        "rawvideo",
        "-pix_fmt",
        "rgb24",
        "-vf",
        &scale_arg(px_w, px_h),
        "-",
    ]);
    let mut child = cmd
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("启动 ffmpeg 失败")?;
    let stdout = child.stdout.take().context("无法获取 ffmpeg stdout")?;
    let stderr = child.stderr.take().context("无法获取 ffmpeg stderr")?;
    Ok(Decoder {
        stdout: BufReader::new(stdout),
        stderr,
        child,
        first_frame: true,
        frame_bytes: (px_w * px_h * 3) as usize,
        px_w,
        px_h,
    })
}

impl Decoder {
    /// 读取一帧到 buf（复用容量）。返回 false 表示播放结束。
    pub fn read_frame(&mut self, buf: &mut Vec<u8>) -> Result<bool> {
        buf.clear();
        buf.resize(self.frame_bytes, 0);
        match self.stdout.read_exact(buf) {
            Ok(()) => {
                self.first_frame = false;
                Ok(true)
            }
            Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
                if self.first_frame {
                    // 一帧都没出就断流：大概率是解码错误，收集 stderr 报告
                    let mut msg = String::new();
                    let _ = self.stderr.read_to_string(&mut msg);
                    let msg = msg.trim();
                    if !msg.is_empty() {
                        bail!("解码失败：{msg}");
                    }
                }
                Ok(false)
            }
            Err(e) => Err(e).context("读取视频帧失败"),
        }
    }
}

impl Drop for Decoder {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rate_parsing() {
        assert_eq!(parse_rate("30000/1001"), Some(29.970_029_970_029_97));
        assert_eq!(parse_rate("24/1"), Some(24.0));
        assert_eq!(parse_rate("0/0"), None);
        assert_eq!(parse_rate("garbage"), None);
    }

    #[test]
    fn scale_argument() {
        assert_eq!(scale_arg(80, 48), "scale=80:48");
    }

    #[test]
    fn seek_argument() {
        assert_eq!(seek_arg(0.0), None);
        assert_eq!(seek_arg(-1.0), None);
        assert_eq!(seek_arg(f64::NAN), None);
        assert_eq!(seek_arg(1.5), Some("1.500".to_string()));
        assert_eq!(seek_arg(12.25), Some("12.250".to_string()));
        assert_eq!(seek_arg(0.001), Some("0.001".to_string()));
    }
}
