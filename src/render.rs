//! 帧渲染：RGB24 → 半块（▀）真彩 ANSI。纯函数、零 IO、可单测。
//!
//! 半块技巧：`▀`（上半块）一格画两个像素——前景色 = 上像素，背景色 = 下像素，
//! 垂直分辨率翻倍，正好抵消终端格子"高是宽两倍"的纵横比。

use std::fmt::Write as _;

/// frame 为 width × (cell_rows×2) 像素的 RGB24 字节（每像素 3 字节）。
/// 输出 = cell_rows 行，行间以 \r\n 分隔；调用方负责光标定位与状态栏。
pub fn frame_to_ansi(frame: &[u8], width: usize, cell_rows: usize) -> String {
    let row_bytes = width * 3;
    let mut out = String::with_capacity(width * cell_rows * 40);
    for cy in 0..cell_rows {
        if cy > 0 {
            out.push_str("\x1b[0m\r\n");
        }
        let top = cy * 2 * row_bytes;
        let bot = (cy * 2 + 1) * row_bytes;
        for x in 0..width {
            let t = top + x * 3;
            let b = bot + x * 3;
            let _ = write!(
                out,
                "\x1b[38;2;{};{};{};48;2;{};{};{}m\u{2580}",
                frame[t],
                frame[t + 1],
                frame[t + 2],
                frame[b],
                frame[b + 1],
                frame[b + 2],
            );
        }
    }
    out.push_str("\x1b[0m");
    out
}

#[cfg(test)]
mod tests {
    use super::frame_to_ansi;

    #[test]
    fn one_cell_two_pixels() {
        // 1×2 像素：上红下蓝 → 一个 ▀，前 38;2 红，后 48;2 蓝
        let frame = [255, 0, 0, 0, 0, 255];
        assert_eq!(
            frame_to_ansi(&frame, 1, 1),
            "\x1b[38;2;255;0;0;48;2;0;0;255m\u{2580}\x1b[0m"
        );
    }

    #[test]
    fn two_cell_rows_one_separator() {
        // 1×4 像素 → 2 行，中间一个 \r\n 分隔，末尾统一复位
        let frame = [0, 0, 0, 1, 1, 1, 2, 2, 2, 3, 3, 3];
        assert_eq!(
            frame_to_ansi(&frame, 1, 2),
            "\x1b[38;2;0;0;0;48;2;1;1;1m\u{2580}\x1b[0m\r\n\
             \x1b[38;2;2;2;2;48;2;3;3;3m\u{2580}\x1b[0m"
        );
    }

    #[test]
    fn three_wide_row_layout() {
        // 3×2 像素 → 一行 3 个 ▀，按行主序取像素
        let mut frame = Vec::new();
        for px in 0u8..6 {
            frame.extend_from_slice(&[px, 0, 0]);
        }
        let out = frame_to_ansi(&frame, 3, 1);
        assert_eq!(out.matches("\u{2580}").count(), 3);
        assert!(out.starts_with("\x1b[38;2;0;0;0;48;2;3;0;0m"));
        assert!(out.ends_with("\x1b[0m"));
    }
}
