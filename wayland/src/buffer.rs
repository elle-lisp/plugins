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
            state
                .events
                .push(crate::state::WlEvent::BufferRelease { buffer_id: *buffer_id });
        }
    }
}
