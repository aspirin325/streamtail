use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RetryLimit {
    Unlimited,
    Limited(u32),
}

impl RetryLimit {
    pub fn allows_retry(self, consecutive_failures: u32) -> bool {
        match self {
            Self::Unlimited => true,
            Self::Limited(limit) => consecutive_failures < limit,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Backoff {
    base: Duration,
    max: Duration,
}

impl Backoff {
    pub fn new(base: Duration, max: Duration) -> Self {
        Self { base, max }
    }

    pub fn delay(self, consecutive_failures: u32) -> Duration {
        let exponent = consecutive_failures.saturating_sub(1).min(16);
        let factor = pow2(exponent);
        let delay = self.base.saturating_mul(factor);

        if delay > self.max {
            self.max
        } else {
            delay
        }
    }
}

impl Default for Backoff {
    fn default() -> Self {
        Self {
            base: Duration::from_millis(500),
            max: Duration::from_secs(30),
        }
    }
}

fn pow2(exponent: u32) -> u32 {
    let mut value = 1_u32;

    for _ in 0..exponent {
        value = value.saturating_mul(2);
    }

    value
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn limited_retry_limit_stops_after_limit() {
        let limit = RetryLimit::Limited(2);

        assert!(limit.allows_retry(0));
        assert!(limit.allows_retry(1));
        assert!(!limit.allows_retry(2));
    }

    #[test]
    fn unlimited_retry_limit_never_stops() {
        assert!(RetryLimit::Unlimited.allows_retry(u32::MAX));
    }

    #[test]
    fn backoff_doubles_until_max() {
        let backoff = Backoff::new(Duration::from_secs(1), Duration::from_secs(3));

        assert_eq!(backoff.delay(1), Duration::from_secs(1));
        assert_eq!(backoff.delay(2), Duration::from_secs(2));
        assert_eq!(backoff.delay(3), Duration::from_secs(3));
        assert_eq!(backoff.delay(4), Duration::from_secs(3));
    }
}
