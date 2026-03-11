use crate::backend::Backend;
use std::sync::atomic::Ordering::Relaxed;
use std::sync::{Arc, atomic::AtomicUsize};

pub trait LoadBalancer {
    fn next_backend(&self) -> Option<&Backend>;
}

pub struct RoundRobin {
    pub backends: Arc<Vec<Backend>>,
    pub index: AtomicUsize,
}

impl LoadBalancer for RoundRobin {
    fn next_backend(&self) -> Option<&Backend> {
        let backends = &self.backends;
        let len = self.backends.len();
        (0..len)
            .map(|_| self.index.fetch_add(1, Relaxed) % len)
            .find(|&idx| backends[idx].is_healthy())
            .map(|idx| &backends[idx])
    }
}
