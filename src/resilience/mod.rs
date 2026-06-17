//! Resilience patterns for high availability
//!
//! Provides circuit breakers, retry logic, and other resilience patterns
//! to prevent cascading failures and improve system reliability.

pub mod circuit_breaker;

pub use circuit_breaker::{CircuitBreaker, CircuitBreakerConfig, CircuitBreakerError};
