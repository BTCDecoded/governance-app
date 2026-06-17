//! Circuit Breaker Pattern Implementation
//!
//! Prevents cascading failures by temporarily stopping requests to failing services.
//! Implements three states: Closed (normal), Open (failing), HalfOpen (testing recovery).

use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;
use tracing::{info, warn};

/// Circuit breaker state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CircuitState {
    /// Circuit is closed - requests are allowed
    Closed,
    /// Circuit is open - requests are rejected immediately
    Open,
    /// Circuit is half-open - testing if service has recovered
    HalfOpen,
}

/// Circuit breaker configuration
#[derive(Debug, Clone)]
pub struct CircuitBreakerConfig {
    /// Failure threshold - open circuit after this many failures
    pub failure_threshold: u32,
    /// Success threshold - close circuit after this many successes in half-open state
    pub success_threshold: u32,
    /// Timeout - how long to wait before trying half-open state
    pub timeout: Duration,
    /// Window duration - time window for counting failures
    pub window_duration: Duration,
}

impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        Self {
            failure_threshold: 5,
            success_threshold: 2,
            timeout: Duration::from_secs(60),
            window_duration: Duration::from_secs(60),
        }
    }
}

/// Circuit breaker for protecting external service calls
pub struct CircuitBreaker {
    state: Arc<Mutex<CircuitState>>,
    failures: Arc<Mutex<Vec<Instant>>>,
    successes: Arc<Mutex<u32>>,
    last_failure_time: Arc<Mutex<Option<Instant>>>,
    config: CircuitBreakerConfig,
    name: String,
}

impl CircuitBreaker {
    /// Create a new circuit breaker with default config
    pub fn new(name: impl Into<String>) -> Self {
        Self::with_config(name, CircuitBreakerConfig::default())
    }

    /// Create a new circuit breaker with custom config
    pub fn with_config(name: impl Into<String>, config: CircuitBreakerConfig) -> Self {
        Self {
            state: Arc::new(Mutex::new(CircuitState::Closed)),
            failures: Arc::new(Mutex::new(Vec::new())),
            successes: Arc::new(Mutex::new(0)),
            last_failure_time: Arc::new(Mutex::new(None)),
            config,
            name: name.into(),
        }
    }

    /// Check if request is allowed
    pub async fn is_open(&self) -> bool {
        // Check state first without holding lock for long
        let state = *self.state.lock().await;

        match state {
            CircuitState::Closed => false,
            CircuitState::Open => {
                // Check if timeout has elapsed - transition to half-open
                // Acquire locks in consistent order: last_failure_time, then state, then successes
                let last_failure = *self.last_failure_time.lock().await;
                if let Some(last_failure_time) = last_failure {
                    if last_failure_time.elapsed() >= self.config.timeout {
                        info!(
                            "Circuit breaker '{}' transitioning to half-open state",
                            self.name
                        );
                        // Release last_failure_time lock before acquiring state lock
                        let mut state_guard = self.state.lock().await;
                        // Double-check state hasn't changed
                        if *state_guard == CircuitState::Open {
                            *state_guard = CircuitState::HalfOpen;
                            *self.successes.lock().await = 0;
                            return false; // Allow request in half-open state
                        }
                    }
                }
                true // Reject request in open state
            }
            CircuitState::HalfOpen => false, // Allow request in half-open state
        }
    }

    /// Record a successful call
    pub async fn record_success(&self) {
        let mut state = self.state.lock().await;
        let mut successes = self.successes.lock().await;

        match *state {
            CircuitState::Closed => {
                // Clean up old failures outside the window
                // Release state and successes locks first to avoid potential deadlock
                drop(state);
                drop(successes);
                self.cleanup_old_failures().await;
            }
            CircuitState::HalfOpen => {
                *successes += 1;
                if *successes >= self.config.success_threshold {
                    info!(
                        "Circuit breaker '{}' transitioning to closed state (recovered)",
                        self.name
                    );
                    *state = CircuitState::Closed;
                    *successes = 0;
                    // Release locks before cleanup to avoid deadlock
                    drop(state);
                    drop(successes);
                    self.cleanup_old_failures().await;
                }
            }
            CircuitState::Open => {
                // Should not happen - success in open state
                warn!(
                    "Circuit breaker '{}' recorded success in open state (unexpected)",
                    self.name
                );
            }
        }
    }

    /// Record a failed call
    pub async fn record_failure(&self) {
        let mut state = self.state.lock().await;
        let mut failures = self.failures.lock().await;
        let mut last_failure_time = self.last_failure_time.lock().await;

        let now = Instant::now();
        failures.push(now);
        *last_failure_time = Some(now);

        // Clean up old failures outside the window (inline to avoid deadlock)
        failures
            .retain(|&failure_time| now.duration_since(failure_time) < self.config.window_duration);

        // Re-count failures after cleanup
        let failure_count = failures.len() as u32;

        match *state {
            CircuitState::Closed => {
                if failure_count >= self.config.failure_threshold {
                    warn!(
                        "Circuit breaker '{}' opening after {} failures (threshold: {})",
                        self.name, failure_count, self.config.failure_threshold
                    );
                    *state = CircuitState::Open;
                }
            }
            CircuitState::HalfOpen => {
                // Any failure in half-open state immediately opens the circuit
                warn!(
                    "Circuit breaker '{}' opening after failure in half-open state",
                    self.name
                );
                *state = CircuitState::Open;
                *self.successes.lock().await = 0;
            }
            CircuitState::Open => {
                // Already open, just update failure time
            }
        }
    }

    /// Clean up old failures outside the time window
    async fn cleanup_old_failures(&self) {
        let mut failures = self.failures.lock().await;
        let now = Instant::now();
        failures
            .retain(|&failure_time| now.duration_since(failure_time) < self.config.window_duration);
    }

    /// Get current state
    pub async fn state(&self) -> CircuitState {
        // Check if we need to transition from Open to HalfOpen based on timeout
        // This ensures state() reflects the current state including timeout transitions
        let current_state = *self.state.lock().await;
        if current_state == CircuitState::Open {
            let last_failure = *self.last_failure_time.lock().await;
            if let Some(last_failure_time) = last_failure {
                if last_failure_time.elapsed() >= self.config.timeout {
                    // Release locks before re-acquiring to avoid deadlock
                    let mut state_guard = self.state.lock().await;
                    // Double-check state hasn't changed
                    if *state_guard == CircuitState::Open {
                        *state_guard = CircuitState::HalfOpen;
                        *self.successes.lock().await = 0;
                        return CircuitState::HalfOpen;
                    }
                }
            }
        }
        current_state
    }

    /// Get failure count in current window
    pub async fn failure_count(&self) -> u32 {
        self.cleanup_old_failures().await;
        let failures = self.failures.lock().await;
        failures.len() as u32
    }

    /// Execute a function with circuit breaker protection
    pub async fn call<F, Fut, T, E>(&self, f: F) -> Result<T, CircuitBreakerError<E>>
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = Result<T, E>>,
    {
        // Check if circuit is open
        if self.is_open().await {
            return Err(CircuitBreakerError::CircuitOpen);
        }

        // Execute the function
        match f().await {
            Ok(result) => {
                self.record_success().await;
                Ok(result)
            }
            Err(e) => {
                self.record_failure().await;
                Err(CircuitBreakerError::ServiceError(e))
            }
        }
    }
}

/// Circuit breaker error
#[derive(Debug)]
pub enum CircuitBreakerError<E> {
    /// Circuit is open - request rejected
    CircuitOpen,
    /// Service error
    ServiceError(E),
}

impl<E> std::fmt::Display for CircuitBreakerError<E>
where
    E: std::fmt::Display,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CircuitBreakerError::CircuitOpen => {
                write!(f, "Circuit breaker is open - service unavailable")
            }
            CircuitBreakerError::ServiceError(e) => {
                write!(f, "Service error: {e}")
            }
        }
    }
}

impl<E> std::error::Error for CircuitBreakerError<E>
where
    E: std::error::Error + 'static,
{
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            CircuitBreakerError::CircuitOpen => None,
            CircuitBreakerError::ServiceError(e) => Some(e),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[tokio::test]
    async fn test_circuit_breaker_closed_to_open() {
        let cb = CircuitBreaker::with_config(
            "test",
            CircuitBreakerConfig {
                failure_threshold: 3,
                success_threshold: 2,
                timeout: Duration::from_secs(1),
                window_duration: Duration::from_secs(60),
            },
        );

        // Initially closed
        assert_eq!(cb.state().await, CircuitState::Closed);
        assert!(!cb.is_open().await);

        // Record failures
        for _ in 0..3 {
            cb.record_failure().await;
        }

        // Should be open now
        assert_eq!(cb.state().await, CircuitState::Open);
        assert!(cb.is_open().await);
    }

    #[tokio::test]
    async fn test_circuit_breaker_half_open_recovery() {
        let cb = CircuitBreaker::with_config(
            "test",
            CircuitBreakerConfig {
                failure_threshold: 2,
                success_threshold: 2,
                timeout: Duration::from_millis(100),
                window_duration: Duration::from_secs(60),
            },
        );

        // Open the circuit
        cb.record_failure().await;
        cb.record_failure().await;
        assert_eq!(cb.state().await, CircuitState::Open);

        // Wait for timeout
        tokio::time::sleep(Duration::from_millis(150)).await;

        // Should transition to half-open
        assert!(!cb.is_open().await); // Allows request
        assert_eq!(cb.state().await, CircuitState::HalfOpen);

        // Record successes
        cb.record_success().await;
        cb.record_success().await;

        // Should be closed now
        assert_eq!(cb.state().await, CircuitState::Closed);
    }
}
