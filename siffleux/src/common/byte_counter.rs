use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

#[derive(Debug, Clone)]
pub struct ByteCounter {
    inner: Arc<ByteCounterInner>,
}

#[derive(Debug)]
struct ByteCounterInner {
    parent: Option<ByteCounter>,
    bytes_write: AtomicUsize,
    bytes_read: AtomicUsize,
}

impl ByteCounter {
    pub fn new(parent: Option<ByteCounter>) -> Self {
        ByteCounter {
            inner: Arc::new(ByteCounterInner {
                parent,
                bytes_write: AtomicUsize::new(0),
                bytes_read: AtomicUsize::new(0),
            }),
        }
    }

    pub fn bytes_read(&self) -> usize {
        self.inner.bytes_read.load(Ordering::Relaxed)
    }

    pub fn bytes_write(&self) -> usize {
        self.inner.bytes_write.load(Ordering::Relaxed)
    }

    pub fn add_bytes_read(&self, bytes: usize) {
        if let Some(parent) = self.inner.parent.as_ref() {
            parent.add_bytes_read(bytes);
        }

        self.inner.bytes_read.fetch_add(bytes, Ordering::Relaxed);
    }

    pub fn add_bytes_write(&self, bytes: usize) {
        if let Some(parent) = self.inner.parent.as_ref() {
            parent.add_bytes_write(bytes);
        }

        self.inner.bytes_write.fetch_add(bytes, Ordering::Relaxed);
    }
}
