// The allocator is set up first thing in main: the thread callback goes in with it.
fn astral_allocator_setup() {
    crate::astral_thread_done::install();
}
