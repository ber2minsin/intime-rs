use image::ImageFormat;
use windows::{
    Win32::{
        Foundation::{HWND, RECT},
        Graphics::{
            Direct3D::D3D_DRIVER_TYPE_UNKNOWN,
            Direct3D11::{
                D3D11_CPU_ACCESS_READ, D3D11_CREATE_DEVICE_FLAG, D3D11_MAP_READ,
                D3D11_MAPPED_SUBRESOURCE, D3D11_SDK_VERSION, D3D11_TEXTURE2D_DESC,
                D3D11_USAGE_STAGING, D3D11CreateDevice, ID3D11Device, ID3D11DeviceContext,
                ID3D11Texture2D,
            },
            Dxgi::{
                Common::{DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_SAMPLE_DESC},
                CreateDXGIFactory1, DXGI_ERROR_ACCESS_LOST, DXGI_ERROR_NOT_FOUND,
                DXGI_ERROR_WAIT_TIMEOUT, DXGI_OUTDUPL_FRAME_INFO, IDXGIAdapter, IDXGIAdapter1,
                IDXGIFactory1, IDXGIOutput1, IDXGIOutputDuplication, IDXGIResource,
            },
            Gdi::{HMONITOR, MONITOR_DEFAULTTONEAREST, MonitorFromWindow},
        },
        UI::WindowsAndMessaging::{GetDesktopWindow, GetWindowRect, IsIconic},
    },
    core::Interface,
};

use crate::{CapturedImage, error::PlatformError, frame_buffer::FrameBuffer, traits::ScreenshotSource};

struct OutputCapture {
    device: ID3D11Device,
    context: ID3D11DeviceContext,
    duplication: IDXGIOutputDuplication,
    staging: ID3D11Texture2D,
    pub width: u32,
    pub height: u32,
    pub monitor_handle: HMONITOR,
    pub desktop_rect: RECT,
}

impl OutputCapture {
    fn new(adapter: &IDXGIAdapter1, output: &IDXGIOutput1) -> Result<Self, PlatformError> {
        let output_desc = unsafe { output.GetDesc() }?;
        let monitor_handle = output_desc.Monitor;
        let desktop_rect = output_desc.DesktopCoordinates;

        let adapter_base: IDXGIAdapter = adapter.cast()?;
        let mut device_out: Option<ID3D11Device> = None;
        let mut context_out: Option<ID3D11DeviceContext> = None;

        unsafe {
            D3D11CreateDevice(
                Some(&adapter_base),
                D3D_DRIVER_TYPE_UNKNOWN,
                windows::Win32::Foundation::HMODULE(std::ptr::null_mut()),
                D3D11_CREATE_DEVICE_FLAG(0),
                None,
                D3D11_SDK_VERSION,
                Some(&mut device_out),
                None,
                Some(&mut context_out),
            )?;
        }

        let device = device_out.ok_or_else(|| {
            anyhow::anyhow!("D3D11CreateDevice succeeded but returned null device")
        })?;
        let context = context_out.ok_or_else(|| {
            anyhow::anyhow!("D3D11CreateDevice succeeded but returned null context")
        })?;

        let duplication = unsafe { output.DuplicateOutput(&device) }?;
        let mut dupl_desc = unsafe { duplication.GetDesc() };
        let width = dupl_desc.ModeDesc.Width;
        let height = dupl_desc.ModeDesc.Height;

        let staging_desc = D3D11_TEXTURE2D_DESC {
            Width: width,
            Height: height,
            MipLevels: 1,
            ArraySize: 1,
            Format: DXGI_FORMAT_B8G8R8A8_UNORM,
            SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
            Usage: D3D11_USAGE_STAGING,
            BindFlags: 0,
            CPUAccessFlags: D3D11_CPU_ACCESS_READ.0 as u32,
            MiscFlags: 0,
        };

        let mut staging_out: Option<ID3D11Texture2D> = None;
        unsafe { device.CreateTexture2D(&staging_desc, None, Some(&mut staging_out))? };
        let staging = staging_out
            .ok_or_else(|| anyhow::anyhow!("CreateTexture2D succeeded but returned null"))?;

        Ok(Self {
            device,
            context,
            duplication,
            staging,
            width,
            height,
            monitor_handle,
            desktop_rect,
        })
    }

    fn capture_full(&mut self, buffer: &mut FrameBuffer, timeout_ms: u32) -> Result<bool, PlatformError> {
        buffer.ensure_size(self.width, self.height);

        let mut frame_info = DXGI_OUTDUPL_FRAME_INFO::default();
        let mut resource: Option<IDXGIResource> = None;

        match unsafe {
            self.duplication
                .AcquireNextFrame(timeout_ms, &mut frame_info, &mut resource)
        } {
            Err(ref e) if e.code() == DXGI_ERROR_WAIT_TIMEOUT => return Ok(false),
            Err(ref e) if e.code() == DXGI_ERROR_ACCESS_LOST => return Err(PlatformError::OutputLost),
            Err(e) => return Err(PlatformError::WindowsError(e)),
            Ok(()) => {}
        }

        let desktop_texture: ID3D11Texture2D = resource
            .ok_or_else(|| anyhow::anyhow!("AcquireNextFrame returned a null resource"))?
            .cast()?;

        unsafe {
            self.context.CopyResource(&self.staging, &desktop_texture);
        }

        if let Err(e) = unsafe { self.duplication.ReleaseFrame() } {
            return Err(PlatformError::WindowsError(e));
        }

        let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
        if let Err(e) = unsafe {
            self.context
                .Map(&self.staging, 0, D3D11_MAP_READ, 0, Some(&mut mapped))
        } {
            return Err(PlatformError::WindowsError(e));
        }

        let src = mapped.pData as *const u8;
        let pitch = mapped.RowPitch as usize;
        let row_bytes = self.width as usize * 4;

        for row in 0..self.height as usize {
            unsafe {
                std::ptr::copy_nonoverlapping(
                    src.add(row * pitch),
                    buffer.data.as_mut_ptr().add(row * row_bytes),
                    row_bytes,
                );
            }
        }

        unsafe {
            self.context.Unmap(&self.staging, 0);
        }

        Ok(true)
    }
}

// Clean structure with NO wrappers. Just standard Rust vectors.
pub struct DxgiCapture {
    outputs: Vec<OutputCapture>,
}

impl DxgiCapture {
    pub fn new() -> Result<Self, PlatformError> {
        let factory: IDXGIFactory1 = unsafe { CreateDXGIFactory1()? };
        let mut outputs: Vec<OutputCapture> = Vec::new();

        let mut adapter_idx = 0u32;
        loop {
            let adapter = match unsafe { factory.EnumAdapters1(adapter_idx) } {
                Ok(a) => a,
                Err(ref e) if e.code() == DXGI_ERROR_NOT_FOUND => break,
                Err(e) => return Err(e.into()),
            };

            let mut output_idx = 0u32;
            loop {
                let output = match unsafe { adapter.EnumOutputs(output_idx) } {
                    Ok(o) => o,
                    Err(ref e) if e.code() == DXGI_ERROR_NOT_FOUND => break,
                    Err(e) => return Err(e.into()),
                };

                let output1: IDXGIOutput1 = match output.cast() {
                    Ok(o) => o,
                    Err(_) => {
                        output_idx += 1;
                        continue;
                    }
                };

                if let Ok(cap) = OutputCapture::new(&adapter, &output1) {
                    outputs.push(cap);
                }

                output_idx += 1;
            }

            adapter_idx += 1;
        }

        if outputs.is_empty() {
            return Err(PlatformError::NoMonitors);
        }

        Ok(Self { outputs })
    }

    pub fn reinitialize(&mut self) -> Result<(), PlatformError> {
        *self = Self::new()?;
        Ok(())
    }

    pub fn monitor_count(&self) -> usize {
        self.outputs.len()
    }

    // Changing to &mut self allows seamless internal direct indexing
    fn capture_window(
        &mut self,
        hwnd: HWND,
        buffer: &mut FrameBuffer,
    ) -> Result<bool, PlatformError> {
        if unsafe { IsIconic(hwnd) }.as_bool() {
            return Err(PlatformError::WindowMinimized);
        }

        let mut window_rect = RECT::default();
        if !unsafe { GetWindowRect(hwnd, &mut window_rect) }.is_ok() {
            return Err(PlatformError::InvalidWindow);
        }

        let win_w = window_rect.right - window_rect.left;
        let win_h = window_rect.bottom - window_rect.top;
        if win_w <= 0 || win_h <= 0 {
            return Err(PlatformError::InvalidWindow);
        }

        let hmonitor = unsafe { MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST) };
        if hmonitor == HMONITOR(std::ptr::null_mut()) {
            return Err(PlatformError::WindowNotOnTrackedMonitor);
        }

        let output_idx = self
            .outputs
            .iter()
            .position(|o| o.monitor_handle == hmonitor)
            .ok_or(PlatformError::WindowNotOnTrackedMonitor)?;

        let dr = self.outputs[output_idx].desktop_rect;
        let mon_w = (dr.right - dr.left) as u32;
        let mon_h = (dr.bottom - dr.top) as u32;

        let local_left = (window_rect.left - dr.left).clamp(0, dr.right - dr.left) as u32;
        let local_top = (window_rect.top - dr.top).clamp(0, dr.bottom - dr.top) as u32;
        let local_right = (window_rect.right - dr.left).clamp(0, dr.right - dr.left) as u32;
        let local_bottom = (window_rect.bottom - dr.top).clamp(0, dr.bottom - dr.top) as u32;

        let crop_w = local_right.saturating_sub(local_left);
        let crop_h = local_bottom.saturating_sub(local_top);

        if crop_w == 0 || crop_h == 0 {
            return Err(PlatformError::WindowNotOnTrackedMonitor);
        }

        let mut temp = FrameBuffer::new(mon_w, mon_h);
        
        // This index access now works perfectly because we have &mut self context
        let captured = self.outputs[output_idx].capture_full(&mut temp, 100)?;
        if !captured {
            return Ok(false);
        }

        buffer.ensure_size(crop_w, crop_h);

        let src_stride = mon_w as usize * 4;
        let dst_stride = crop_w as usize * 4;
        let x_offset = local_left as usize * 4;

        for row in 0..crop_h as usize {
            let src_start = (local_top as usize + row) * src_stride + x_offset;
            let dst_start = row * dst_stride;
            buffer.data[dst_start..dst_start + dst_stride]
                .copy_from_slice(&temp.data[src_start..src_start + dst_stride]);
        }

        Ok(true)
    }
}

impl ScreenshotSource for DxgiCapture {
    // Implemented with &mut self to clean up ownership issues
    fn capture(&mut self, handle: u64) -> Result<CapturedImage, PlatformError> {
        let mut buffer = FrameBuffer::new(1, 1);
        let hwnd = HWND(handle as usize as *mut std::ffi::c_void);

        let _ = self.capture_window(hwnd, &mut buffer)?;

        let width = buffer.width;
        let height = buffer.height;
        let raw_pixels = buffer.data;

        if width > 0 && height > 0 && !raw_pixels.is_empty() {
            let mut rgb_pixels = Vec::with_capacity((width * height * 3) as usize);

            for pixel in raw_pixels.chunks_exact(4) {
                rgb_pixels.push(pixel[2]); // R
                rgb_pixels.push(pixel[1]); // G
                rgb_pixels.push(pixel[0]); // B
            }

            Ok(CapturedImage { width, height, pixels: rgb_pixels })
        } else {
            Err(PlatformError::EmptyFrame)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::ImageFormat;
    use windows::Win32::UI::WindowsAndMessaging::GetDesktopWindow;

    #[test]
    fn smoke_primary_monitor() -> Result<(), PlatformError> {
        let mut capture = DxgiCapture::new()?;
        println!("Tracking {} monitor(s)", capture.monitor_count());

        let mut buffer = FrameBuffer::new(1, 1);
        let hwnd = unsafe { GetDesktopWindow() };

        let _ = capture.capture_window(hwnd, &mut buffer);

        // --- SAVE AS JPEG ---
        let width = buffer.width;
        let height = buffer.height;
        let mut raw_pixels = buffer.data;

        if width > 0 && height > 0 && !raw_pixels.is_empty() {
            // Pre-allocate a vector for RGB data (3 bytes per pixel instead of 4)
            let mut rgb_pixels = Vec::with_capacity((width * height * 3) as usize);

            // Swap BGRA to RGB by stripping the Alpha channel
            for pixel in raw_pixels.chunks_exact(4) {
                rgb_pixels.push(pixel[2]); // R (originally at index 2)
                rgb_pixels.push(pixel[1]); // G (originally at index 1)
                rgb_pixels.push(pixel[0]); // B (originally at index 0)
            }

            // Wrap into a native RgbImage container (no alpha channel)
            let img = image::RgbImage::from_raw(width, height, rgb_pixels)
                .expect("Failed to create RGB image buffer from raw data");

            // Save image as Jpeg (JPEG natively supports RGB8)
            img.save_with_format("test_capture.jpg", ImageFormat::Jpeg)
                .expect("Failed to save image file");

            println!("Saved capture verification to test_capture.jpg");
        } else {
            println!("Capture returned an empty buffer!");
        }
        Ok(())
    }
}
