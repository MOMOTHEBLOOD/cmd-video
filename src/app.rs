//! 应用层：终端装配、事件循环、状态栏。渲染与解码都委托给下层模块。

use std::io::{self, BufWriter, Write};
use std::path::Path;
use std::time::{Duration, Instant};

use anyhow::{bail, Context, Result};
use crossterm::cursor::{Hide, Show};
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use crossterm::terminal::{self, EnterAlternateScreen, LeaveAlternateScreen};
use crossterm::ExecutableCommand as _;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr as _};

use crate::ff::{self, Decoder, VideoMeta};
use crate::player::{fmt_time, Clock, State};
use crate::render::frame_to_ansi;

#[derive(Debug, Clone)]
pub struct Config {
    /// 帧率上限（终端刷屏开销大，超过 20~24fps 意义不大）
    pub fps_cap: f64,
    /// 手动指定的 ffmpeg 工具目录
    pub ffmpeg_dir: Option<std::path::PathBuf>,
}

pub fn run(input: &Path, cfg: &Config) -> Result<()> {
    let bin_dir = ff::find_bin_dir(cfg.ffmpeg_dir.as_deref())?;
    let meta = ff::probe(&bin_dir, input)?;

    let mut stdout = io::stdout();
    terminal::enable_raw_mode().context("启用终端 raw mode 失败")?;
    stdout
        .execute(EnterAlternateScreen)
        .context("进入备用屏失败")?;
    stdout.execute(Hide).context("隐藏光标失败")?;

    // panic 时尽量恢复终端
    let orig_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = terminal::disable_raw_mode();
        let mut s = io::stdout();
        let _ = s.execute(Show);
        let _ = s.execute(LeaveAlternateScreen);
        orig_hook(info);
    }));

    let res = play(&mut stdout, input, &bin_dir, meta, cfg);

    stdout.execute(Show)?;
    stdout.execute(LeaveAlternateScreen)?;
    terminal::disable_raw_mode()?;
    res
}

fn play<W: Write>(
    w: &mut W,
    input: &Path,
    bin_dir: &Path,
    meta: VideoMeta,
    cfg: &Config,
) -> Result<()> {
    let (mut cols, mut rows) = terminal::size().context("无法获取终端尺寸")?;
    if cols < 20 || rows < 6 {
        bail!("终端太小（{cols}x{rows}），至少需要 20x6");
    }
    let fps = meta.fps.min(cfg.fps_cap).max(1.0);
    let mut clock = Clock::new(fps, 0.0);
    let mut dec = open_decoder(bin_dir, input, 0.0, cols, rows)?;
    let mut buf: Vec<u8> = Vec::with_capacity(dec.frame_bytes);
    let mut state = State::Playing;
    let mut out = BufWriter::new(w);

    loop {
        // ---- 事件轮询：等到下一帧该渲染的时刻（暂停/结束时每 50ms 醒一次）----
        let wait = match state {
            State::Playing => clock
                .next_due()
                .saturating_duration_since(Instant::now())
                .min(Duration::from_millis(50)),
            _ => Duration::from_millis(50),
        };
        if event::poll(wait)? {
            match event::read()? {
                Event::Key(k) if k.kind == KeyEventKind::Press => {
                    if handle_key(&k, &mut state, &mut clock)? {
                        break;
                    }
                    if let Some(target) = seek_requested(&k, &clock) {
                        let target = clamp_target(target, meta.duration);
                        dec = open_decoder(bin_dir, input, target, cols, rows)?;
                        buf.resize(dec.frame_bytes, 0);
                        clock.seek(target);
                        state = State::Playing;
                    }
                }
                Event::Resize(c, r) => {
                    (cols, rows) = (c, r);
                    let pos = clock.position();
                    dec = open_decoder(bin_dir, input, pos, cols, rows)?;
                    buf.resize(dec.frame_bytes, 0);
                    clock.seek(pos);
                }
                _ => {}
            }
        }

        // ---- 帧渲染：到点才画，落后则自动追帧 ----
        if state == State::Playing && Instant::now() >= clock.next_due() {
            if !dec.read_frame(&mut buf)? {
                state = State::Ended;
            } else {
                let ansi = frame_to_ansi(&buf, dec.width as usize, dec.cell_rows as usize);
                out.write_all(b"\x1b[1;1H")?;
                out.write_all(ansi.as_bytes())?;
                clock.advance();
            }
        }

        // ---- 状态栏 ----
        let dur = meta
            .duration
            .map(fmt_time)
            .unwrap_or_else(|| "--:--".into());
        let icon = match state {
            State::Playing => "\u{25b6}",
            State::Paused => "\u{2016}",
            State::Ended => "\u{25a0}",
        };
        let hint = match state {
            State::Ended => "Space 重播  ←→ ±10s  q 退出",
            State::Paused => "Space 继续  ←→ ±10s  q 退出",
            State::Playing => "Space 暂停  ←→ ±10s  q 退出",
        };
        let status = format!(
            " {icon} {}  {} / {}  {:.0}fps  {hint} ",
            display_name(input),
            fmt_time(clock.position()),
            dur,
            clock.fps(),
        );
        let status = truncate_width(&status, cols as usize);
        write!(out, "\x1b[{rows};1H\x1b[0K{status}")?;
        out.flush()?;
    }
    Ok(())
}

/// 处理退出/暂停键。返回 true 表示退出。
fn handle_key(k: &KeyEvent, state: &mut State, clock: &mut Clock) -> Result<bool> {
    if k.modifiers.contains(KeyModifiers::CONTROL) && k.code == KeyCode::Char('c') {
        return Ok(true);
    }
    match k.code {
        KeyCode::Char('q') | KeyCode::Esc => return Ok(true),
        KeyCode::Char(' ') => match *state {
            State::Playing => *state = State::Paused,
            State::Paused => {
                *state = State::Playing;
                clock.reanchor();
            }
            State::Ended => {
                clock.seek(0.0);
                *state = State::Playing;
            }
        },
        _ => {}
    }
    Ok(false)
}

/// 左右方向键产生跳转目标
fn seek_requested(k: &KeyEvent, clock: &Clock) -> Option<f64> {
    match k.code {
        KeyCode::Left => Some(clock.target_after(-10.0)),
        KeyCode::Right => Some(clock.target_after(10.0)),
        _ => None,
    }
}

fn clamp_target(target: f64, duration: Option<f64>) -> f64 {
    let mut t = target.max(0.0);
    if let Some(d) = duration {
        t = t.min((d - 0.25).max(0.0));
    }
    t
}

fn open_decoder(bin_dir: &Path, input: &Path, seek: f64, cols: u16, rows: u16) -> Result<Decoder> {
    let width = cols.clamp(20, 400) as u32;
    let cell_rows = rows.saturating_sub(1).clamp(3, 200) as u32;
    ff::spawn_decoder(bin_dir, input, seek, width, cell_rows)
}

fn display_name(input: &Path) -> String {
    input
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| input.display().to_string())
}

fn truncate_width(s: &str, max: usize) -> String {
    if s.width() <= max {
        return s.to_owned();
    }
    let mut out = String::new();
    let mut w = 0usize;
    for c in s.chars() {
        let cw = UnicodeWidthChar::width(c).unwrap_or(0);
        if w + cw > max.saturating_sub(1) {
            break;
        }
        out.push(c);
        w += cw;
    }
    out.push('\u{2026}');
    out
}
