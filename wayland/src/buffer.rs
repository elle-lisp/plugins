//! SHM buffer management — memfd, mmap, write, fill.

use wayland_client::protocol::{wl_buffer, wl_shm, wl_shm_pool};
use wayland_client::{Connection, Dispatch, QueueHandle};

use crate::state::WaylandState;

// ── Buffer tracking ───────────────────────────────────────────────────

pub struct ShmBuffer {
    pub id: u32,
    pub buffer: wl_buffer::WlBuffer,
    pub data: *mut u8,
    pub size: usize,
    pub width: i32,
    pub height: i32,
    pub stride: i32,
}

impl ShmBuffer {
    /// Create a new SHM buffer backed by a memfd.
    pub fn create(
        shm: &wl_shm::WlShm,
        qh: &QueueHandle<WaylandState>,
        id: u32,
        width: i32,
        height: i32,
    ) -> Result<Self, String> {
        let stride = width * 4; // ARGB8888
        let size = (stride * height) as usize;

        // Create memfd
        let name = std::ffi::CString::new("elle-wl-shm").unwrap();
        let fd = unsafe { libc::memfd_create(name.as_ptr(), libc::MFD_CLOEXEC) };
        if fd < 0 {
            return Err("memfd_create failed".into());
        }

        // Resize
        if unsafe { libc::ftruncate(fd, size as libc::off_t) } < 0 {
            unsafe { libc::close(fd) };
            return Err("ftruncate failed".into());
        }

        // mmap
        let data = unsafe {
            libc::mmap(
                std::ptr::null_mut(),
                size,
                libc::PROT_READ | libc::PROT_WRITE,
                libc::MAP_SHARED,
                fd,
                0,
            )
        };
        if data == libc::MAP_FAILED {
            unsafe { libc::close(fd) };
            return Err("mmap failed".into());
        }

        // Create wl_shm_pool + wl_buffer
        use std::os::unix::io::BorrowedFd;
        let borrowed_fd = unsafe { BorrowedFd::borrow_raw(fd) };
        let pool = shm.create_pool(borrowed_fd, size as i32, qh, ());
        let buffer = pool.create_buffer(0, width, height, stride, wl_shm::Format::Argb8888, qh, id);
        pool.destroy();
        unsafe { libc::close(fd) };

        Ok(ShmBuffer {
            id,
            buffer,
            data: data as *mut u8,
            size,
            width,
            height,
            stride,
        })
    }

    /// Fill the entire buffer with a single ARGB color.
    pub fn fill(&mut self, argb: u32) {
        let pixels = self.size / 4;
        let ptr = self.data as *mut u32;
        for i in 0..pixels {
            unsafe { *ptr.add(i) = argb };
        }
    }

    /// Fill a rectangular region with a single ARGB color.
    /// Coordinates are clamped to buffer bounds.
    pub fn fill_rect(&mut self, x: i32, y: i32, w: i32, h: i32, argb: u32) {
        let x0 = x.max(0) as usize;
        let y0 = y.max(0) as usize;
        let x1 = (x + w).min(self.width) as usize;
        let y1 = (y + h).min(self.height) as usize;
        let stride_pixels = self.stride as usize / 4;
        for row in y0..y1 {
            for col in x0..x1 {
                unsafe {
                    *((self.data as *mut u32).add(row * stride_pixels + col)) = argb;
                };
            }
        }
    }

    /// Fill a circle with a single ARGB color.
    /// (cx, cy) is the center, r is the radius.
    pub fn fill_circle(&mut self, cx: i32, cy: i32, r: i32, argb: u32) {
        if r <= 0 {
            return;
        }
        let r2 = (r * r) as i64;
        let stride_pixels = self.stride as usize / 4;
        let dy_min = (-r).max(-cy);
        let dy_max = r.min(self.height - 1 - cy);
        for dy in dy_min..=dy_max {
            let row = (cy + dy) as usize;
            let dx_max = ((r2 - (dy as i64) * (dy as i64)) as f64).sqrt() as i32;
            let x0 = (cx - dx_max).max(0) as usize;
            let x1 = (cx + dx_max).min(self.width - 1) as usize;
            let base = row * stride_pixels;
            for col in x0..=x1 {
                unsafe {
                    *((self.data as *mut u32).add(base + col)) = argb;
                };
            }
        }
    }

    /// Fill a triangle with a single ARGB color.
    /// Vertices (x1,y1), (x2,y2), (x3,y3).
    /// Uses scanline fill with edge interpolation.
    #[allow(clippy::too_many_arguments)]
    pub fn fill_triangle(
        &mut self,
        x1: i32,
        y1: i32,
        x2: i32,
        y2: i32,
        x3: i32,
        y3: i32,
        argb: u32,
    ) {
        // Sort vertices by y
        let mut verts = [(x1, y1), (x2, y2), (x3, y3)];
        verts.sort_by_key(|&(_, y)| y);
        let (ax, ay) = verts[0];
        let (bx, by) = verts[1];
        let (cx, cy) = verts[2];

        let stride_pixels = self.stride as usize / 4;

        // Helper: fill one scanline
        let fill_span = |y: i32, xa: i32, xb: i32| {
            if y < 0 || y >= self.height {
                return;
            }
            let (x0, x1) = if xa < xb { (xa, xb) } else { (xb, xa) };
            let x0 = x0.max(0) as usize;
            let x1 = x1.min(self.width - 1) as usize;
            let base = (y as usize) * stride_pixels;
            for col in x0..=x1 {
                unsafe {
                    *((self.data as *mut u32).add(base + col)) = argb;
                };
            }
        };

        // Interpolate x along an edge from (x0,y0) to (x1,y1) at given y
        let edge_x = |x0: i32, y0: i32, x1: i32, y1: i32, y: i32| -> i32 {
            if y1 == y0 {
                return x0;
            }
            x0 + ((x1 - x0) as i64 * (y - y0) as i64 / (y1 - y0) as i64) as i32
        };

        // Scan from top vertex to middle vertex
        if ay != by {
            for y in ay..by {
                let xa = edge_x(ax, ay, bx, by, y);
                let xb = edge_x(ax, ay, cx, cy, y);
                fill_span(y, xa, xb);
            }
        }
        // Scan from middle vertex to bottom vertex
        if by != cy {
            for y in by..=cy {
                let xa = edge_x(bx, by, cx, cy, y);
                let xb = edge_x(ax, ay, cx, cy, y);
                fill_span(y, xa, xb);
            }
        }
    }

    /// Write raw bytes to the buffer at an offset.
    pub fn write(&mut self, offset: usize, data: &[u8]) -> Result<(), String> {
        if offset + data.len() > self.size {
            return Err("write would exceed buffer size".into());
        }
        unsafe {
            std::ptr::copy_nonoverlapping(data.as_ptr(), self.data.add(offset), data.len());
        }
        Ok(())
    }
}

impl Drop for ShmBuffer {
    fn drop(&mut self) {
        unsafe {
            libc::munmap(self.data as *mut libc::c_void, self.size);
        }
        self.buffer.destroy();
    }
}

// ── Dispatch impls ────────────────────────────────────────────────────

impl Dispatch<wl_shm_pool::WlShmPool, ()> for WaylandState {
    fn event(
        _state: &mut Self,
        _pool: &wl_shm_pool::WlShmPool,
        _event: wl_shm_pool::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<WaylandState>,
    ) {
    }
}

impl Dispatch<wl_buffer::WlBuffer, u32> for WaylandState {
    fn event(
        state: &mut Self,
        _buffer: &wl_buffer::WlBuffer,
        event: wl_buffer::Event,
        buffer_id: &u32,
        _conn: &Connection,
        _qh: &QueueHandle<WaylandState>,
    ) {
        if let wl_buffer::Event::Release = event {
            state.events.push(crate::state::WlEvent::BufferRelease {
                buffer_id: *buffer_id,
            });
        }
    }
}
