//! GPU health watchdog — detects GPU device loss via a lightweight D3D11 sentinel device.
//!
//! Creates a minimal D3D11 device (no swap chain, no rendering) on the same adapter
//! that wgpu uses, then polls `GetDeviceRemovedReason()` every few seconds. When the
//! GPU driver resets (TDR), the device is removed, or the driver crashes, this thread
//! logs the exact DXGI error code to `iced-crash.log` — giving us hard evidence for
//! crashes that bypass Rust's panic handler and the SEH exception filter.
//!
//! Uses raw FFI to avoid adding new crate dependencies.

use crate::crash_handler;
use std::ffi::c_void;

// --- HRESULT codes ---
const S_OK: i32 = 0;
const DXGI_ERROR_DEVICE_REMOVED: i32 = -0x7FF9_FFB4i32; // 0x887A0005
const DXGI_ERROR_DEVICE_HUNG: i32 = -0x7FF9_FFBAi32; // 0x887A0006
const DXGI_ERROR_DEVICE_RESET: i32 = -0x7FF9_FFB9i32; // 0x887A0007
const DXGI_ERROR_DRIVER_INTERNAL_ERROR: i32 = -0x7FF9_FFBEi32; // 0x887A0020
const DXGI_ERROR_INVALID_CALL: i32 = -0x7FF9_FFB8i32; // 0x887A0001

// D3D11 constants
const D3D_DRIVER_TYPE_HARDWARE: u32 = 1;
const D3D11_SDK_VERSION: u32 = 7;

// COM vtable indices for ID3D11Device (inherits IUnknown: 0=QueryInterface, 1=AddRef, 2=Release)
const VTABLE_RELEASE: usize = 2;
const VTABLE_GET_DEVICE_REMOVED_REASON: usize = 39;

type D3D11CreateDeviceFn = unsafe extern "system" fn(
    p_adapter: *mut c_void,       // IDXGIAdapter*
    driver_type: u32,             // D3D_DRIVER_TYPE
    software: *mut c_void,        // HMODULE
    flags: u32,                   // UINT
    p_feature_levels: *const u32, // const D3D_FEATURE_LEVEL*
    feature_levels: u32,          // UINT
    sdk_version: u32,             // UINT
    pp_device: *mut *mut c_void,  // ID3D11Device**
    p_feature_level: *mut u32,    // D3D_FEATURE_LEVEL*
    pp_context: *mut *mut c_void, // ID3D11DeviceContext**
) -> i32;

extern "system" {
    fn LoadLibraryA(name: *const u8) -> *mut c_void;
    fn GetProcAddress(module: *mut c_void, name: *const u8) -> *mut c_void;
}

/// Call a COM method by vtable index on an ID3D11Device pointer.
/// Safety: `device` must be a valid COM object pointer.
unsafe fn com_call_no_args(device: *mut c_void, vtable_index: usize) -> i32 {
    let vtable = *(device as *const *const usize);
    let method: unsafe extern "system" fn(*mut c_void) -> i32 =
        std::mem::transmute(*vtable.add(vtable_index));
    method(device)
}

/// Release a COM object (IUnknown::Release).
unsafe fn com_release(obj: *mut c_void) {
    let vtable = *(obj as *const *const usize);
    let release: unsafe extern "system" fn(*mut c_void) -> u32 =
        std::mem::transmute(*vtable.add(VTABLE_RELEASE));
    release(obj);
}

fn dxgi_error_name(hr: i32) -> &'static str {
    match hr {
        _ if hr == DXGI_ERROR_DEVICE_REMOVED => "DXGI_ERROR_DEVICE_REMOVED",
        _ if hr == DXGI_ERROR_DEVICE_HUNG => "DXGI_ERROR_DEVICE_HUNG",
        _ if hr == DXGI_ERROR_DEVICE_RESET => "DXGI_ERROR_DEVICE_RESET",
        _ if hr == DXGI_ERROR_DRIVER_INTERNAL_ERROR => "DXGI_ERROR_DRIVER_INTERNAL_ERROR",
        _ if hr == DXGI_ERROR_INVALID_CALL => "DXGI_ERROR_INVALID_CALL",
        _ => "UNKNOWN_DXGI_ERROR",
    }
}

/// Spawn the GPU watchdog thread. Returns immediately.
/// The thread creates a minimal D3D11 device and polls its health every 2 seconds.
/// On failure to load D3D11 or create a device, logs a warning and exits silently.
#[cfg(windows)]
pub fn start() {
    std::thread::Builder::new()
        .name("gpu-watchdog".into())
        .spawn(|| {
            if let Err(e) = watchdog_loop() {
                crash_handler::log(&format!("GPU_WATCHDOG: failed to start — {e}"));
            }
        })
        .ok();
}

#[cfg(not(windows))]
pub fn start() {
    // No-op on non-Windows
}

fn watchdog_loop() -> Result<(), String> {
    // Dynamically load d3d11.dll
    let module = unsafe { LoadLibraryA(b"d3d11.dll\0".as_ptr()) };
    if module.is_null() {
        return Err("d3d11.dll not found".into());
    }

    let create_device_ptr =
        unsafe { GetProcAddress(module, b"D3D11CreateDevice\0".as_ptr()) };
    if create_device_ptr.is_null() {
        return Err("D3D11CreateDevice not found in d3d11.dll".into());
    }

    let create_device: D3D11CreateDeviceFn = unsafe { std::mem::transmute(create_device_ptr) };

    // Create a minimal device — no swap chain, no rendering context needed
    let mut device: *mut c_void = std::ptr::null_mut();
    let mut context: *mut c_void = std::ptr::null_mut();
    let hr = unsafe {
        create_device(
            std::ptr::null_mut(), // default adapter (same one wgpu picks)
            D3D_DRIVER_TYPE_HARDWARE,
            std::ptr::null_mut(),
            0, // no flags
            std::ptr::null(),
            0, // default feature levels
            D3D11_SDK_VERSION,
            &mut device,
            std::ptr::null_mut(),
            &mut context,
        )
    };

    if hr != S_OK || device.is_null() {
        return Err(format!("D3D11CreateDevice failed: HRESULT=0x{:08X}", hr as u32));
    }

    // Release the immediate context — we don't need it
    if !context.is_null() {
        unsafe { com_release(context) };
    }

    crash_handler::log("GPU_WATCHDOG: sentinel D3D11 device created, monitoring started");

    // Poll loop — check every 2 seconds
    loop {
        std::thread::sleep(std::time::Duration::from_secs(2));

        let reason =
            unsafe { com_call_no_args(device, VTABLE_GET_DEVICE_REMOVED_REASON) };

        if reason != S_OK {
            let name = dxgi_error_name(reason);
            crash_handler::log(&format!(
                "GPU_WATCHDOG: *** DEVICE LOST *** reason=0x{:08X} ({})",
                reason as u32, name
            ));

            // Log a few more times in case the process is about to die
            for i in 1..=3 {
                crash_handler::log(&format!(
                    "GPU_WATCHDOG: device lost confirmation #{i} — 0x{:08X} ({name})",
                    reason as u32
                ));
                std::thread::sleep(std::time::Duration::from_millis(100));
            }

            // Release and exit — the device is dead
            unsafe { com_release(device) };
            return Ok(());
        }
    }
}
