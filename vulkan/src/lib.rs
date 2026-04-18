mod context;
mod decode;
mod dispatch;
mod shader;

use context::GpuCtx;
use dispatch::{BufferSpec, BufferUsage, DispatchBuffer, GpuBuffer, GpuHandle};
use shader::GpuShader;

use elle_plugin::{ElleResult, ElleValue, EllePrimDef, SIG_ERROR, SIG_IO, SIG_OK, SIG_YIELD};

elle_plugin::define_plugin!("vulkan/", &PRIMITIVES);

// ── Helpers ─────────────────────────────────────────────────────

fn get_ctx<'a>(val: ElleValue, name: &str) -> Result<&'a GpuCtx, ElleResult> {
    let a = api();
    a.get_external::<GpuCtx>(val, "vulkan-ctx")
        .ok_or_else(|| a.err("type-error", &format!("{name}: expected vulkan-ctx, got {}", a.type_name(val))))
}

fn get_shader<'a>(val: ElleValue, name: &str) -> Result<&'a GpuShader, ElleResult> {
    let a = api();
    a.get_external::<GpuShader>(val, "vulkan-shader")
        .ok_or_else(|| a.err("type-error", &format!("{name}: expected vulkan-shader, got {}", a.type_name(val))))
}

fn extract_keyword(val: ElleValue) -> Option<String> {
    let a = api();
    a.get_keyword_name(val).map(|s| s.to_string())
}

fn struct_get(val: ElleValue, key: &str) -> Option<ElleValue> {
    let a = api();
    let field = a.get_struct_field(val, key);
    if a.check_nil(field) {
        None
    } else {
        Some(field)
    }
}

// ── vulkan/init ─────────────────────────────────────────────────

extern "C" fn prim_init(_args: *const ElleValue, _nargs: usize) -> ElleResult {
    let a = api();
    match context::init_vulkan() {
        Ok(ctx) => a.ok(a.external("vulkan-ctx", ctx)),
        Err(msg) => a.err("gpu-error", &msg),
    }
}

// ── vulkan/shader ───────────────────────────────────────────────

extern "C" fn prim_shader(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    let ctx = match get_ctx(unsafe { a.arg(args, nargs, 0) }, "vulkan/shader") {
        Ok(c) => c,
        Err(e) => return e,
    };
    let spirv_val = unsafe { a.arg(args, nargs, 1) };
    let num_buf_val = unsafe { a.arg(args, nargs, 2) };
    let num_buffers = match a.get_int(num_buf_val) {
        Some(n) if n > 0 => n as u32,
        _ => return a.err("value-error", "vulkan/shader: num-buffers must be positive"),
    };

    let spirv = if let Some(b) = a.get_bytes(spirv_val) {
        b.to_vec()
    } else if let Some(path) = a.get_string(spirv_val) {
        match std::fs::read(path) {
            Ok(b) => b,
            Err(e) => return a.err("io-error", &format!("vulkan/shader: {e}")),
        }
    } else {
        return a.err(
            "type-error",
            &format!("vulkan/shader: expected bytes or path string, got {}", a.type_name(spirv_val)),
        );
    };

    match shader::create_shader(&ctx.inner, &spirv, num_buffers) {
        Ok(s) => a.ok(a.external("vulkan-shader", s)),
        Err(msg) => a.err("gpu-error", &msg),
    }
}

// ── vulkan/dispatch ─────────────────────────────────────────────

extern "C" fn prim_dispatch(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    let shader = match get_shader(unsafe { a.arg(args, nargs, 0) }, "vulkan/dispatch") {
        Ok(s) => s,
        Err(e) => return e,
    };

    let wg = |i: usize, name: &str| -> Result<u32, ElleResult> {
        match a.get_int(unsafe { a.arg(args, nargs, i) }) {
            Some(n) if n > 0 => Ok(n as u32),
            _ => Err(a.err("value-error", &format!("vulkan/dispatch: {name} must be positive"))),
        }
    };
    let wg_x = match wg(1, "wg-x") { Ok(v) => v, Err(e) => return e };
    let wg_y = match wg(2, "wg-y") { Ok(v) => v, Err(e) => return e };
    let wg_z = match wg(3, "wg-z") { Ok(v) => v, Err(e) => return e };

    let bufs_val = unsafe { a.arg(args, nargs, 4) };
    let buf_len = match a.get_array_len(bufs_val) {
        Some(n) => n,
        None => return a.err("type-error", "vulkan/dispatch: buffers must be an array"),
    };

    let mut dbufs = Vec::with_capacity(buf_len);
    for i in 0..buf_len {
        let spec_val = a.get_array_item(bufs_val, i);
        match parse_dispatch_buffer(spec_val, i, "vulkan/dispatch") {
            Ok(db) => dbufs.push(db),
            Err(e) => return e,
        }
    }

    if dbufs.len() != shader.num_buffers as usize {
        return a.err(
            "value-error",
            &format!(
                "vulkan/dispatch: shader expects {} buffers, got {}",
                shader.num_buffers, dbufs.len()
            ),
        );
    }

    match dispatch::dispatch(
        shader.ctx.clone(),
        shader.pipeline,
        shader.pipeline_layout,
        shader.descriptor_set_layout,
        [wg_x, wg_y, wg_z],
        dbufs,
    ) {
        Ok(handle) => a.ok(a.external("vulkan-handle", handle)),
        Err(msg) => a.err("gpu-error", &msg),
    }
}

// ── vulkan/wait ─────────────────────────────────────────────────

extern "C" fn prim_wait(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    let handle_val = unsafe { a.arg(args, nargs, 0) };
    let handle = match a.get_external::<GpuHandle>(handle_val, "vulkan-handle") {
        Some(h) => h,
        None => {
            return a.err(
                "type-error",
                &format!("vulkan/wait: expected vulkan-handle, got {}", a.type_name(handle_val)),
            );
        }
    };
    let fd = handle.fence_fd;
    ElleResult {
        signal: SIG_YIELD | SIG_IO,
        value: a.poll_fd(fd, libc::POLLIN as u32),
    }
}

// ── vulkan/collect ──────────────────────────────────────────────

extern "C" fn prim_collect(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    let handle_val = unsafe { a.arg(args, nargs, 0) };
    let handle = match a.get_external::<GpuHandle>(handle_val, "vulkan-handle") {
        Some(h) => h,
        None => {
            return a.err(
                "type-error",
                &format!("vulkan/collect: expected vulkan-handle, got {}", a.type_name(handle_val)),
            );
        }
    };

    match dispatch::collect_ref(handle) {
        Ok(bytes) => a.ok(a.bytes(&bytes)),
        Err(msg) => a.err("gpu-error", &msg),
    }
}

// ── vulkan/submit (convenience: dispatch + wait + collect) ──────
// NOTE: IoRequest::task is not available through stable ABI. This primitive
// is kept as a synchronous blocking call on the current thread.

extern "C" fn prim_submit(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    let shader = match get_shader(unsafe { a.arg(args, nargs, 0) }, "vulkan/submit") {
        Ok(s) => s,
        Err(e) => return e,
    };

    let wg = |i: usize, name: &str| -> Result<u32, ElleResult> {
        match a.get_int(unsafe { a.arg(args, nargs, i) }) {
            Some(n) if n > 0 => Ok(n as u32),
            _ => Err(a.err("value-error", &format!("vulkan/submit: {name} must be positive"))),
        }
    };
    let wg_x = match wg(1, "wg-x") { Ok(v) => v, Err(e) => return e };
    let wg_y = match wg(2, "wg-y") { Ok(v) => v, Err(e) => return e };
    let wg_z = match wg(3, "wg-z") { Ok(v) => v, Err(e) => return e };

    let bufs_val = unsafe { a.arg(args, nargs, 4) };
    let buf_len = match a.get_array_len(bufs_val) {
        Some(n) => n,
        None => return a.err("type-error", "vulkan/submit: buffers must be an array"),
    };

    let mut dbufs = Vec::with_capacity(buf_len);
    for i in 0..buf_len {
        let spec_val = a.get_array_item(bufs_val, i);
        match parse_dispatch_buffer(spec_val, i, "vulkan/submit") {
            Ok(db) => dbufs.push(db),
            Err(e) => return e,
        }
    }

    if dbufs.len() != shader.num_buffers as usize {
        return a.err(
            "value-error",
            &format!(
                "vulkan/submit: shader expects {} buffers, got {}",
                shader.num_buffers, dbufs.len()
            ),
        );
    }

    let ctx_arc = shader.ctx.clone();
    let pipeline = shader.pipeline;
    let pipeline_layout = shader.pipeline_layout;
    let descriptor_set_layout = shader.descriptor_set_layout;

    match dispatch::dispatch(
        ctx_arc.clone(),
        pipeline,
        pipeline_layout,
        descriptor_set_layout,
        [wg_x, wg_y, wg_z],
        dbufs,
    ) {
        Ok(handle) => {
            // Block on fence synchronously
            let state = ctx_arc.lock().unwrap();
            unsafe {
                state
                    .device
                    .wait_for_fences(&[handle.fence], true, u64::MAX)
            }
            .ok();
            drop(state);
            match dispatch::collect_ref(&handle) {
                Ok(bytes) => a.ok(a.bytes(&bytes)),
                Err(msg) => a.err("gpu-error", &msg),
            }
        }
        Err(msg) => a.err("gpu-error", &msg),
    }
}

/// Parse a dispatch buffer: either a persistent GpuBuffer or a fresh BufferSpec.
fn parse_dispatch_buffer(
    val: ElleValue,
    index: usize,
    caller: &str,
) -> Result<DispatchBuffer, ElleResult> {
    let a = api();
    if let Some(gpu_buf) = a.get_external::<GpuBuffer>(val, "vulkan-buffer") {
        return Ok(DispatchBuffer::Persistent {
            buffer: gpu_buf.buffer,
            byte_size: gpu_buf.byte_size,
            usage: BufferUsage::Input,
        });
    }
    parse_buffer_spec(val, index, caller).map(DispatchBuffer::Spec)
}

fn parse_buffer_spec(
    val: ElleValue,
    index: usize,
    caller: &str,
) -> Result<BufferSpec, ElleResult> {
    let a = api();

    let usage_val = struct_get(val, "usage").ok_or_else(|| {
        a.err("value-error", &format!("{caller}: buffer[{index}] missing :usage"))
    })?;

    let usage = match extract_keyword(usage_val) {
        Some(ref k) if k == "input" => BufferUsage::Input,
        Some(ref k) if k == "output" => BufferUsage::Output,
        Some(ref k) if k == "inout" => BufferUsage::InOut,
        _ => {
            return Err(a.err(
                "value-error",
                &format!("{caller}: buffer[{index}] :usage must be :input, :output, or :inout"),
            ));
        }
    };

    if usage == BufferUsage::Output {
        let size_val = struct_get(val, "size").ok_or_else(|| {
            a.err("value-error", &format!("{caller}: output buffer[{index}] missing :size"))
        })?;
        let byte_size = a.get_int(size_val).ok_or_else(|| {
            a.err("type-error", &format!("{caller}: buffer[{index}] :size must be integer"))
        })? as usize;
        return Ok(BufferSpec {
            data: Vec::new(),
            byte_size,
            usage,
        });
    }

    let data_val = struct_get(val, "data").ok_or_else(|| {
        a.err("value-error", &format!("{caller}: buffer[{index}] missing :data"))
    })?;

    let arr_len = a.get_array_len(data_val).ok_or_else(|| {
        a.err("type-error", &format!("{caller}: buffer[{index}] :data must be an array"))
    })?;

    let dtype = struct_get(val, "dtype")
        .and_then(|v| extract_keyword(v))
        .unwrap_or_else(|| "f32".to_string());

    let elem_size = if dtype == "i64" { 8 } else { 4 };
    let mut bytes = Vec::with_capacity(arr_len * elem_size);
    for j in 0..arr_len {
        let v = a.get_array_item(data_val, j);
        match dtype.as_str() {
            "f32" => {
                let f = if let Some(f) = a.get_float(v) { f as f32 }
                    else if let Some(i) = a.get_int(v) { i as f32 }
                    else {
                        return Err(a.err("type-error",
                            &format!("{caller}: buffer[{index}][{j}] must be numeric, got {}", a.type_name(v))));
                    };
                bytes.extend_from_slice(&f.to_le_bytes());
            }
            "u32" => {
                let n = a.get_int(v)
                    .ok_or_else(|| a.err("type-error",
                        &format!("{caller}: buffer[{index}][{j}] must be integer for :u32")))?;
                bytes.extend_from_slice(&(n as u32).to_le_bytes());
            }
            "i32" => {
                let n = a.get_int(v)
                    .ok_or_else(|| a.err("type-error",
                        &format!("{caller}: buffer[{index}][{j}] must be integer for :i32")))?;
                bytes.extend_from_slice(&(n as i32).to_le_bytes());
            }
            "i64" => {
                let n = a.get_int(v)
                    .ok_or_else(|| a.err("type-error",
                        &format!("{caller}: buffer[{index}][{j}] must be integer for :i64")))?;
                bytes.extend_from_slice(&n.to_le_bytes());
            }
            _ => return Err(a.err("value-error",
                &format!("{caller}: buffer[{index}] unsupported :dtype {dtype:?}, expected :f32, :u32, :i32, or :i64"))),
        }
    }

    let byte_size = bytes.len();
    Ok(BufferSpec {
        data: bytes,
        byte_size,
        usage,
    })
}

// ── vulkan/decode ───────────────────────────────────────────────

extern "C" fn prim_decode(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    let bytes_val = unsafe { a.arg(args, nargs, 0) };
    let bytes = match a.get_bytes(bytes_val) {
        Some(b) => b.to_vec(),
        None => {
            return a.err(
                "type-error",
                &format!("vulkan/decode: expected bytes, got {}", a.type_name(bytes_val)),
            );
        }
    };

    let dtype_val = unsafe { a.arg(args, nargs, 1) };
    let dtype = match extract_keyword(dtype_val) {
        Some(k) if matches!(k.as_str(), "f32" | "u32" | "i32" | "i64" | "raw") => k,
        _ => {
            return a.err(
                "value-error",
                "vulkan/decode: dtype must be :f32, :u32, :i32, :i64, or :raw",
            );
        }
    };

    match decode::decode(&bytes, &dtype) {
        Ok(val) => a.ok(val),
        Err(msg) => a.err("gpu-error", &msg),
    }
}

// ── vulkan/f32-bits ─────────────────────────────────────────────

extern "C" fn prim_f32_bits(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    let val = unsafe { a.arg(args, nargs, 0) };
    let f = if let Some(f) = a.get_float(val) {
        f
    } else if let Some(i) = a.get_int(val) {
        i as f64
    } else {
        return a.err(
            "type-error",
            &format!("vulkan/f32-bits: expected number, got {}", a.type_name(val)),
        );
    };
    let bits = (f as f32).to_bits();
    a.ok(a.int(bits as i64))
}

// ── vulkan/persist ──────────────────────────────────────────────

extern "C" fn prim_persist(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    let ctx = match get_ctx(unsafe { a.arg(args, nargs, 0) }, "vulkan/persist") {
        Ok(c) => c,
        Err(e) => return e,
    };

    let spec = match parse_buffer_spec(unsafe { a.arg(args, nargs, 1) }, 0, "vulkan/persist") {
        Ok(s) => s,
        Err(e) => return e,
    };

    let location = match spec.usage {
        BufferUsage::Input => gpu_allocator::MemoryLocation::CpuToGpu,
        BufferUsage::Output => gpu_allocator::MemoryLocation::GpuToCpu,
        BufferUsage::InOut => gpu_allocator::MemoryLocation::CpuToGpu,
    };

    let mut state = match ctx.inner.lock() {
        Ok(s) => s,
        Err(e) => return a.err("gpu-error", &format!("lock: {e}")),
    };

    let (buffer, allocation) = match state.acquire_buffer(spec.byte_size, location, 0) {
        Ok(ba) => ba,
        Err(msg) => return a.err("gpu-error", &msg),
    };

    let gpu_buf = GpuBuffer {
        ctx: ctx.inner.clone(),
        buffer,
        allocation,
        byte_size: spec.byte_size,
        location,
    };

    if !spec.data.is_empty() {
        if let Err(msg) = gpu_buf.upload(&spec.data) {
            return a.err("gpu-error", &msg);
        }
    }

    a.ok(a.external("vulkan-buffer", gpu_buf))
}

// ── vulkan/update ──────────────────────────────────────────────

extern "C" fn prim_update(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    let buf_val = unsafe { a.arg(args, nargs, 0) };
    let gpu_buf = match a.get_external::<GpuBuffer>(buf_val, "vulkan-buffer") {
        Some(b) => b,
        None => {
            return a.err(
                "type-error",
                &format!("vulkan/update: expected vulkan-buffer, got {}", a.type_name(buf_val)),
            );
        }
    };

    let spec = match parse_buffer_spec(unsafe { a.arg(args, nargs, 1) }, 0, "vulkan/update") {
        Ok(s) => s,
        Err(e) => return e,
    };

    match gpu_buf.upload(&spec.data) {
        Ok(()) => a.ok(a.nil()),
        Err(msg) => a.err("gpu-error", &msg),
    }
}

// ── Primitive table ─────────────────────────────────────────────

static PRIMITIVES: &[EllePrimDef] = &[
    EllePrimDef::exact("vulkan/init", prim_init, SIG_ERROR, 0, "Initialize Vulkan GPU context", "gpu", "(vulkan/init)"),
    EllePrimDef::exact("vulkan/shader", prim_shader, SIG_ERROR, 3, "Load SPIR-V shader from file path, create compute pipeline", "gpu", "(vulkan/shader ctx \"shader.spv\" 3)"),
    EllePrimDef::exact("vulkan/dispatch", prim_dispatch, SIG_ERROR, 5, "Submit GPU compute work, return handle (non-blocking)", "gpu", "(vulkan/dispatch shader 4 1 1 bufs)"),
    EllePrimDef::exact("vulkan/wait", prim_wait, SIG_ERROR | SIG_YIELD | SIG_IO, 1, "Wait for GPU dispatch to complete (fiber suspends on fence fd)", "gpu", "(vulkan/wait handle)"),
    EllePrimDef::exact("vulkan/collect", prim_collect, SIG_ERROR, 1, "Read back results after GPU completes", "gpu", "(vulkan/collect handle)"),
    EllePrimDef::exact("vulkan/submit", prim_submit, SIG_ERROR, 5, "Dispatch + wait + collect in one call (blocking)", "gpu", "(vulkan/submit shader 4 1 1 bufs)"),
    EllePrimDef::exact("vulkan/f32-bits", prim_f32_bits, SIG_ERROR, 1, "Return IEEE 754 f32 bit pattern of a number as integer", "gpu", "(vulkan/f32-bits 1.0)"),
    EllePrimDef::exact("vulkan/decode", prim_decode, SIG_ERROR, 2, "Decode GPU result bytes to Elle float arrays", "gpu", "(vulkan/decode result :f32)"),
    EllePrimDef::exact("vulkan/persist", prim_persist, SIG_ERROR, 2, "Create a persistent GPU buffer from a buffer spec", "gpu", "(vulkan/persist ctx (gpu:input data))"),
    EllePrimDef::exact("vulkan/update", prim_update, SIG_ERROR, 2, "Re-upload data to a persistent GPU buffer", "gpu", "(vulkan/update buf (gpu:input new-data))"),
];
