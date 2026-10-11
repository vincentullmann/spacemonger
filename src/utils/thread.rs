//! Thread helpers.

/// Drop `value` on a short-lived background thread. Freeing a tree of millions of entries
/// takes long enough to stall a frame; nothing waits for it here.
pub fn drop_in_background<T: Send + 'static>(value: T) {
    // If the thread can't be started, `value` comes back in the error and is dropped here.
    let _ = std::thread::Builder::new()
        .name("drop".into())
        .spawn(move || drop(value));
}
