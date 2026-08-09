//! Tiny retained software scene used by the native shell.
//!
//! This deliberately mirrors the Lege Viewer split between scene construction
//! and software presentation, while remaining small enough for an AKR records
//! workbench. Every frame is rebuilt only after an input or load event.

use font8x8::{BASIC_FONTS, UnicodeFonts};

#[derive(Debug, Clone)]
pub struct Canvas {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u32>,
}

impl Canvas {
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            pixels: vec![0; width.saturating_mul(height) as usize],
        }
    }
    pub fn resize(&mut self, width: u32, height: u32) {
        self.width = width;
        self.height = height;
        self.pixels.resize(width.saturating_mul(height) as usize, 0);
    }
    pub fn clear(&mut self, color: u32) {
        self.pixels.fill(color);
    }
    pub fn rect(&mut self, x: i32, y: i32, width: i32, height: i32, color: u32) {
        let x0 = x.max(0) as u32;
        let y0 = y.max(0) as u32;
        let x1 = (x.saturating_add(width)).max(0).min(self.width as i32) as u32;
        let y1 = (y.saturating_add(height)).max(0).min(self.height as i32) as u32;
        for row in y0..y1 {
            let start = (row * self.width + x0) as usize;
            let end = (row * self.width + x1) as usize;
            self.pixels[start..end].fill(color);
        }
    }
    pub fn border(&mut self, x: i32, y: i32, width: i32, height: i32, color: u32) {
        self.rect(x, y, width, 1, color);
        self.rect(x, y + height - 1, width, 1, color);
        self.rect(x, y, 1, height, color);
        self.rect(x + width - 1, y, 1, height, color);
    }
    pub fn text(&mut self, x: i32, y: i32, text: &str, color: u32) {
        for (index, character) in text.chars().enumerate() {
            self.glyph(x + index as i32 * 8, y, character, color);
        }
    }
    pub fn text_clipped(&mut self, x: i32, y: i32, width: i32, text: &str, color: u32) {
        let count = width.max(0) as usize / 8;
        let clipped = if text.chars().count() > count && count >= 3 {
            format!("{}...", text.chars().take(count - 3).collect::<String>())
        } else {
            text.chars().take(count).collect()
        };
        self.text(x, y, &clipped, color);
    }
    fn glyph(&mut self, x: i32, y: i32, character: char, color: u32) {
        let Some(glyph) = BASIC_FONTS.get(character) else {
            return;
        };
        for (row, bits) in glyph.iter().enumerate() {
            for column in 0..8 {
                if bits & (1 << column) != 0 {
                    self.rect(x + column, y + row as i32, 1, 1, color);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn drawing_clips_outside_the_surface() {
        let mut canvas = Canvas::new(4, 4);
        canvas.clear(1);
        canvas.rect(-2, -2, 4, 4, 2);
        assert_eq!(canvas.pixels.iter().filter(|pixel| **pixel == 2).count(), 4);
    }
    #[test]
    fn clipped_text_has_bounded_width() {
        let mut canvas = Canvas::new(32, 8);
        canvas.text_clipped(0, 0, 16, "long label", 1);
        assert!(canvas.pixels.contains(&1));
    }
}
