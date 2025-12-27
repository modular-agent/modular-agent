use std::sync::atomic::AtomicUsize;

static ID_COUNTER: AtomicUsize = AtomicUsize::new(1);

pub(crate) fn new_id() -> String {
    return ID_COUNTER
        .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        .to_string();
}
