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
    /// Integer glyph magnification. The bitmap font is 8x8, so every text
    /// metric in the shell is derived from this instead of being hard-coded.
    pub scale: i32,
}

impl Canvas {
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            pixels: vec![0; width.saturating_mul(height) as usize],
            scale: 1,
        }
    }
    pub fn set_scale(&mut self, scale: i32) {
        self.scale = scale.clamp(1, 6);
    }
    /// Advance width of one glyph at the current scale.
    pub fn char_width(&self) -> i32 {
        8 * self.scale
    }
    /// Cell height of one glyph at the current scale.
    pub fn char_height(&self) -> i32 {
        8 * self.scale
    }
    /// How many whole glyphs fit in `width` pixels.
    pub fn columns(&self, width: i32) -> usize {
        (width.max(0) / self.char_width()).max(0) as usize
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
        if y + self.char_height() < 0 || y > self.height as i32 {
            return;
        }
        let advance = self.char_width();
        for (index, character) in text.chars().enumerate() {
            self.glyph(x + index as i32 * advance, y, character, color);
        }
    }
    pub fn text_clipped(&mut self, x: i32, y: i32, width: i32, text: &str, color: u32) {
        let count = self.columns(width);
        let clipped = if text.chars().count() > count && count >= 3 {
            format!("{}...", text.chars().take(count - 3).collect::<String>())
        } else {
            text.chars().take(count).collect()
        };
        self.text(x, y, &clipped, color);
    }
    fn glyph(&mut self, x: i32, y: i32, character: char, color: u32) {
        if character == ' ' {
            return;
        }
        let glyph = match BASIC_FONTS.get(character) {
            Some(glyph) => glyph,
            // Keep unsupported code points visible rather than silently
            // swallowing them, so unusual record text still reads as text.
            None => match BASIC_FONTS.get('?') {
                Some(glyph) => glyph,
                None => return,
            },
        };
        let scale = self.scale;
        for (row, bits) in glyph.iter().enumerate() {
            for column in 0..8 {
                if bits & (1 << column) != 0 {
                    self.rect(
                        x + column * scale,
                        y + row as i32 * scale,
                        scale,
                        scale,
                        color,
                    );
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
    #[test]
    fn metrics_follow_the_glyph_scale() {
        let mut canvas = Canvas::new(64, 64);
        assert_eq!((canvas.char_width(), canvas.columns(64)), (8, 8));
        canvas.set_scale(2);
        assert_eq!((canvas.char_width(), canvas.columns(64)), (16, 4));
        canvas.set_scale(99);
        assert_eq!(canvas.scale, 6);
    }
    #[test]
    fn scaled_glyphs_cover_more_pixels() {
        let mut small = Canvas::new(64, 64);
        small.text(0, 0, "A", 1);
        let mut large = Canvas::new(64, 64);
        large.set_scale(2);
        large.text(0, 0, "A", 1);
        let count = |canvas: &Canvas| canvas.pixels.iter().filter(|pixel| **pixel == 1).count();
        assert_eq!(count(&large), count(&small) * 4);
    }
}
