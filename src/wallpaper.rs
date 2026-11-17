//! 毛玻璃底：读桌面壁纸、按「填充」方式裁到窗口位置、糊掉，做成面板的背景。

use eframe::egui::{Color32, ColorImage, Context, TextureHandle, TextureOptions};

pub struct Backdrop {
    pub texture: TextureHandle,
    pub size: [usize; 2],
}

impl Backdrop {
    /// 从壁纸文件生成窗口区域的模糊底图。
    /// `screen` 是主屏物理尺寸，`rect` 是窗口在屏幕上的位置与大小。
    pub fn from_wallpaper(
        ctx: &Context,
        path: &str,
        screen: (i32, i32),
        rect: (i32, i32, i32, i32),
    ) -> Option<Self> {
        let image = image::open(path).ok()?.to_rgb8();
        let (iw, ih) = (image.width() as f32, image.height() as f32);
        let (sw, sh) = (screen.0 as f32, screen.1 as f32);
        // Windows「填充」：等比放大到覆盖整屏，再居中裁切。
        let scale = (sw / iw).max(sh / ih);
        let draw_w = iw * scale;
        let draw_h = ih * scale;
        let offset_x = (sw - draw_w) / 2.0;
        let offset_y = (sh - draw_h) / 2.0;

        let (rx, ry, rw, rh) = (
            rect.0 as f32,
            rect.1 as f32,
            rect.2.max(1) as f32,
            rect.3.max(1) as f32,
        );
        let step = 10.0f32;
        let cols = ((rw / step) as usize).max(8);
        let rows = ((rh / step) as usize).max(8);
        let mut small = vec![[0f32; 3]; cols * rows];
        for row in 0..rows {
            for col in 0..cols {
                let screen_x = rx + col as f32 * step + step / 2.0;
                let screen_y = ry + row as f32 * step + step / 2.0;
                let sx = ((screen_x - offset_x) / scale).clamp(0.0, iw - 1.0) as u32;
                let sy = ((screen_y - offset_y) / scale).clamp(0.0, ih - 1.0) as u32;
                let pixel = image.get_pixel(sx, sy);
                small[row * cols + col] = [
                    pixel[0] as f32,
                    pixel[1] as f32,
                    pixel[2] as f32,
                ];
            }
        }
        for _ in 0..4 {
            box_blur(&mut small, cols, rows);
        }
        let pixels: Vec<Color32> = small
            .iter()
            .map(|c| {
                Color32::from_rgb(
                    c[0].clamp(0.0, 255.0) as u8,
                    c[1].clamp(0.0, 255.0) as u8,
                    c[2].clamp(0.0, 255.0) as u8,
                )
            })
            .collect();
        let texture = ctx.load_texture(
            "wallpaper-glass",
            ColorImage {
                size: [cols, rows],
                pixels,
            },
            TextureOptions::LINEAR,
        );
        Some(Self {
            texture,
            size: [cols, rows],
        })
    }
}

fn box_blur(pixels: &mut [[f32; 3]], width: usize, height: usize) {
    let mut buffer = pixels.to_vec();
    for y in 0..height {
        for x in 0..width {
            let mut sum = [0f32; 3];
            let mut count = 0f32;
            for dy in -1i32..=1 {
                for dx in -1i32..=1 {
                    let nx = x as i32 + dx;
                    let ny = y as i32 + dy;
                    if nx < 0 || ny < 0 || nx >= width as i32 || ny >= height as i32 {
                        continue;
                    }
                    let slot = ny as usize * width + nx as usize;
                    for channel in 0..3 {
                        sum[channel] += pixels[slot][channel];
                    }
                    count += 1.0;
                }
            }
            for channel in 0..3 {
                buffer[y * width + x][channel] = sum[channel] / count;
            }
        }
    }
    pixels.copy_from_slice(&buffer);
}
