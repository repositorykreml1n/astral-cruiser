// mimalloc tells a thread's end by a TLS callback set up with pragmas only MSVC
// reads. Built with MinGW, as the launcher builds a native game, it never runs:
// each finished thread keeps its segments, 4 MB and more, for good, and the game
// starts short-lived threads all the time. So this sets the callback up itself.
#[cfg(all(windows, target_env = "gnu"))]
unsafe extern "system" fn on_thread(_: *mut std::ffi::c_void, reason: u32, _: *mut std::ffi::c_void) {
    const DLL_THREAD_DETACH: u32 = 3;
    if reason == DLL_THREAD_DETACH {
        // SAFETY: the thread is ending, after Rust's own thread locals (.CRT$XLB).
        unsafe { libmimalloc_sys::mi_thread_done() };
    }
}

#[cfg(all(windows, target_env = "gnu"))]
#[used]
#[unsafe(link_section = ".CRT$XLY")]
static ON_THREAD: unsafe extern "system" fn(*mut std::ffi::c_void, u32, *mut std::ffi::c_void) = on_thread;

/// Keeps the callback in the program: nothing else refers to it.
pub fn install() {
    #[cfg(all(windows, target_env = "gnu"))]
    std::hint::black_box(&ON_THREAD);
}
