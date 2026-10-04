//! 播放时钟：墙钟 + 帧号推进。暂停/跳转只改锚点，纯逻辑可单测。
//!
//! 模型：内容时间 = content_base + frames_played / fps；
//! 下一帧该显示的时刻 = wall_base + frames_played / fps（墙钟）。
//! 渲染慢导致落后时 due 已过，循环自然不再 sleep——自动丢帧同步。

use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    Playing,
    Paused,
    Ended,
}

#[derive(Debug)]
pub struct Clock {
    fps: f64,
    frames_played: u64,
    /// wall_base 时刻对应的内容时间（秒）
    content_base: f64,
    wall_base: Instant,
}

impl Clock {
    pub fn new(fps: f64, content_base: f64) -> Self {
        debug_assert!(fps > 0.0);
        Self {
            fps,
            frames_played: 0,
            content_base,
            wall_base: Instant::now(),
        }
    }

    pub fn fps(&self) -> f64 {
        self.fps
    }

    pub fn frames_played(&self) -> u64 {
        self.frames_played
    }

    /// 播放头的内容时间（秒）
    pub fn position(&self) -> f64 {
        self.content_base + self.frames_played as f64 / self.fps
    }

    /// 下一帧应渲染的墙钟时刻
    pub fn next_due(&self) -> Instant {
        self.wall_base + Duration::from_secs_f64(self.frames_played as f64 / self.fps)
    }

    /// 渲染完一帧后推进
    pub fn advance(&mut self) {
        self.frames_played += 1;
    }

    /// 暂停恢复（或终端尺寸变化）后重新锚定墙钟，消除停顿期间的漂移
    pub fn reanchor(&mut self) {
        self.wall_base = Instant::now();
    }

    /// 跳转到内容时间 t（秒），并重锚墙钟
    pub fn seek(&mut self, t: f64) {
        self.frames_played = 0;
        self.content_base = t.max(0.0);
        self.wall_base = Instant::now();
    }

    /// 快进/快退 ±delta 秒的目标位置
    pub fn target_after(&self, delta: f64) -> f64 {
        (self.position() + delta).max(0.0)
    }
}

pub fn fmt_time(secs: f64) -> String {
    let s = secs.max(0.0) as u64;
    format!("{:02}:{:02}", s / 60, s % 60)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn time_formatting() {
        assert_eq!(fmt_time(0.0), "00:00");
        assert_eq!(fmt_time(75.4), "01:15");
        assert_eq!(fmt_time(-3.0), "00:00");
        assert_eq!(fmt_time(3671.0), "61:11");
    }

    #[test]
    fn position_advances_with_frames() {
        let mut c = Clock::new(20.0, 0.0);
        assert_eq!(c.position(), 0.0);
        for _ in 0..20 {
            c.advance();
        }
        assert!((c.position() - 1.0).abs() < 1e-9);
    }

    #[test]
    fn seek_resets_and_clamps() {
        let mut c = Clock::new(10.0, 30.0);
        c.advance();
        c.seek(15.0);
        assert_eq!(c.position(), 15.0);
        assert_eq!(c.frames_played(), 0);
        c.seek(-5.0);
        assert_eq!(c.position(), 0.0);
    }

    #[test]
    fn target_after_clamps_at_zero() {
        let c = Clock::new(10.0, 5.0);
        assert_eq!(c.target_after(-100.0), 0.0);
        assert_eq!(c.target_after(10.0), 15.0);
    }

    #[test]
    fn next_due_grows_with_frames() {
        let mut c = Clock::new(25.0, 0.0);
        let due0 = c.next_due();
        c.advance();
        assert!(c.next_due() > due0);
    }
}
