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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allocates_bgra_buffer() {
        let fb = FrameBuffer::new(2, 3);
        assert_eq!(fb.width, 2);
        assert_eq!(fb.height, 3);
        assert_eq!(fb.data.len(), 2 * 3 * 4);
        assert!(fb.data.iter().all(|b| *b == 0));
    }

    #[test]
    fn ensure_size_is_noop_when_unchanged() {
        let mut fb = FrameBuffer::new(4, 4);
        let ptr = fb.data.as_ptr();
        fb.ensure_size(4, 4);
        assert_eq!(fb.data.as_ptr(), ptr);
    }

    #[test]
    fn ensure_size_resizes() {
        let mut fb = FrameBuffer::new(1, 1);
        fb.data[0] = 9;
        fb.ensure_size(2, 2);
        assert_eq!(fb.width, 2);
        assert_eq!(fb.height, 2);
        assert_eq!(fb.data.len(), 16);
    }
}
