
pub struct FrameBuffer {
    /// Raw BGRA8 pixels, `width * height * 4` bytes.
    pub data: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

impl FrameBuffer {
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            data: vec![0u8; (width * height * 4) as usize],
            width,
            height,
        }
    }

    /// Resize in-place. No-op (and no reallocation) when dimensions match.
    pub fn ensure_size(&mut self, width: u32, height: u32) {
        if self.width != width || self.height != height {
            self.width = width;
            self.height = height;
            self.data.resize((width * height * 4) as usize, 0);
        }
    }
}