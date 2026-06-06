use dashmap::DashMap;
use std::time::{Duration, Instant};

pub struct RateLimiter {
    ip_counts: DashMap<String, (u32, Instant)>,
    session_counts: DashMap<String, (u32, Instant)>,
    max_per_ip: u32,
    max_per_session: u32,
    window: Duration,
}

impl RateLimiter {
    pub fn new(max_per_ip: u32, max_per_session: u32, window: Duration) -> Self {
        Self {
            ip_counts: DashMap::new(),
            session_counts: DashMap::new(),
            max_per_ip,
            max_per_session,
            window,
        }
    }

    pub fn check_and_increment_ip(&self, ip: &str) -> bool {
        let mut entry = self
            .ip_counts
            .entry(ip.to_string())
            .or_insert((0, Instant::now()));

        if entry.1.elapsed() > self.window {
            entry.0 = 1;
            entry.1 = Instant::now();
            true
        } else if entry.0 < self.max_per_ip {
            entry.0 += 1;
            true
        } else {
            false
        }
    }

    pub fn check_and_increment_session(&self, session: &str) -> bool {
        let mut entry = self
            .session_counts
            .entry(session.to_string())
            .or_insert((0, Instant::now()));

        if entry.1.elapsed() > self.window {
            entry.0 = 1;
            entry.1 = Instant::now();
            true
        } else if entry.0 < self.max_per_session {
            entry.0 += 1;
            true
        } else {
            false
        }
    }
}
