use std::{
    net::SocketAddr,
    sync::atomic::{AtomicBool, AtomicU32, Ordering::Relaxed},
};

pub struct Backend {
    pub addr: SocketAddr,
    pub healthy: AtomicBool,
    pub successes: AtomicU32,
    pub failures: AtomicU32,
    pub failure_threshold: u32,
    pub recovery_threshold: u32,
}

impl Backend {
    pub fn new(addr: SocketAddr, failure_threshold: u32, recovery_threshold: u32) -> Self {
        Self {
            addr,
            healthy: AtomicBool::new(true),
            successes: AtomicU32::new(0),
            failures: AtomicU32::new(0),
            failure_threshold,
            recovery_threshold,
        }
    }

    pub fn is_healthy(&self) -> bool {
        self.healthy.load(Relaxed)
    }

    pub fn record_success(&self) {
        if self.successes.fetch_add(1, Relaxed) + 1 >= self.recovery_threshold {
            self.healthy.store(true, Relaxed);
        }
        self.failures.store(0, Relaxed);
    }

    pub fn record_failure(&self) {
        if self.failures.fetch_add(1, Relaxed) + 1 >= self.failure_threshold {
            self.healthy.store(false, Relaxed);
        }
        self.successes.store(0, Relaxed);
    }
}
