//! Circuit breaker pattern for resilient browser applications.
//!
//! Tracks consecutive failures and temporarily pauses operations to allow
//! a failing Streamline server to recover. The browser's `performance.now()`
//! is used for timing.
//!
//! # Example (JavaScript)
//!
//! ```javascript
//! import { CircuitBreaker } from '@streamlinelabs/streamline-wasm-sdk';
//!
//! const cb = new CircuitBreaker(5, 2, 30000);
//!
//! if (cb.allow()) {
//!     try {
//!         await doOperation();
//!         cb.record_success();
//!     } catch (e) {
//!         if (e.retryable) cb.record_failure();
//!     }
//! } else {
//!     console.log("Circuit open — backing off");
//! }
//! ```

use wasm_bindgen::prelude::*;

/// Circuit breaker states.
#[wasm_bindgen]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CircuitState {
    /// Normal operation — requests flow through.
    Closed,
    /// Too many failures — requests are rejected immediately.
    Open,
    /// Recovery probe — a limited number of requests are allowed through.
    HalfOpen,
}

/// Circuit breaker that protects browser applications from cascading failures.
///
/// Transitions:
///   Closed → Open        when `failure_threshold` consecutive failures occur
///   Open → HalfOpen      when `open_timeout_ms` elapses
///   HalfOpen → Closed    when `success_threshold` consecutive successes occur
///   HalfOpen → Open      on any failure during probing
#[wasm_bindgen]
pub struct CircuitBreaker {
    state: CircuitState,
    failure_threshold: u32,
    success_threshold: u32,
    open_timeout_ms: f64,
    half_open_max_requests: u32,

    failure_count: u32,
    success_count: u32,
    half_open_count: u32,
    last_failure_at: f64,
    last_state_change: f64,
}

#[wasm_bindgen]
impl CircuitBreaker {
    /// Create a new circuit breaker.
    ///
    /// - `failure_threshold`: consecutive failures before opening (default: 5)
    /// - `success_threshold`: consecutive successes in half-open to close (default: 2)
    /// - `open_timeout_ms`: milliseconds to wait before probing (default: 30000)
    #[wasm_bindgen(constructor)]
    pub fn new(failure_threshold: u32, success_threshold: u32, open_timeout_ms: f64) -> Self {
        let now = now_ms();
        Self {
            state: CircuitState::Closed,
            failure_threshold: if failure_threshold == 0 {
                5
            } else {
                failure_threshold
            },
            success_threshold: if success_threshold == 0 {
                2
            } else {
                success_threshold
            },
            open_timeout_ms: if open_timeout_ms <= 0.0 {
                30_000.0
            } else {
                open_timeout_ms
            },
            half_open_max_requests: 3,
            failure_count: 0,
            success_count: 0,
            half_open_count: 0,
            last_failure_at: 0.0,
            last_state_change: now,
        }
    }

    /// Check whether a request should be allowed through.
    pub fn allow(&mut self) -> bool {
        match self.state {
            CircuitState::Closed => true,
            CircuitState::Open => {
                let elapsed = now_ms() - self.last_failure_at;
                if elapsed >= self.open_timeout_ms {
                    self.transition(CircuitState::HalfOpen);
                    self.half_open_count = 1;
                    true
                } else {
                    false
                }
            }
            CircuitState::HalfOpen => {
                if self.half_open_count < self.half_open_max_requests {
                    self.half_open_count += 1;
                    true
                } else {
                    false
                }
            }
        }
    }

    /// Record a successful operation. Call after every successful request.
    pub fn record_success(&mut self) {
        match self.state {
            CircuitState::Closed => {
                self.failure_count = 0;
            }
            CircuitState::HalfOpen => {
                self.success_count += 1;
                if self.success_count >= self.success_threshold {
                    self.transition(CircuitState::Closed);
                }
            }
            CircuitState::Open => {}
        }
    }

    /// Record a failed operation. Only call for *retryable* errors —
    /// non-retryable errors (auth, not found) should not trip the breaker.
    pub fn record_failure(&mut self) {
        self.last_failure_at = now_ms();

        match self.state {
            CircuitState::Closed => {
                self.failure_count += 1;
                if self.failure_count >= self.failure_threshold {
                    self.transition(CircuitState::Open);
                }
            }
            CircuitState::HalfOpen => {
                self.transition(CircuitState::Open);
            }
            CircuitState::Open => {}
        }
    }

    /// Get the current circuit state.
    #[wasm_bindgen(getter)]
    pub fn state(&mut self) -> CircuitState {
        // Auto-transition from open to half-open if timeout elapsed
        if self.state == CircuitState::Open {
            let elapsed = now_ms() - self.last_failure_at;
            if elapsed >= self.open_timeout_ms {
                self.transition(CircuitState::HalfOpen);
            }
        }
        self.state
    }

    /// Get the current failure count.
    #[wasm_bindgen(getter)]
    pub fn failure_count(&self) -> u32 {
        self.failure_count
    }

    /// Get the current success count.
    #[wasm_bindgen(getter)]
    pub fn success_count(&self) -> u32 {
        self.success_count
    }

    /// Manually reset the circuit breaker to Closed state.
    pub fn reset(&mut self) {
        self.transition(CircuitState::Closed);
    }
}

impl CircuitBreaker {
    fn transition(&mut self, to: CircuitState) {
        if self.state == to {
            return;
        }
        self.state = to;
        self.last_state_change = now_ms();
        self.failure_count = 0;
        self.success_count = 0;
        self.half_open_count = 0;
    }
}

/// Get current time in milliseconds. Uses `performance.now()` when available
/// (browser), falls back to `Date.now()`, and finally to `0.0` (tests).
fn now_ms() -> f64 {
    #[cfg(target_arch = "wasm32")]
    {
        js_sys::Reflect::get(&js_sys::global(), &"performance".into())
            .ok()
            .and_then(|perf| {
                if perf.is_undefined() {
                    None
                } else {
                    js_sys::Reflect::get(&perf, &"now".into())
                        .ok()
                        .and_then(|f| {
                            let func: js_sys::Function = f.dyn_into().ok()?;
                            func.call0(&perf).ok()?.as_f64()
                        })
                }
            })
            .unwrap_or_else(|| js_sys::Date::now())
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        // For native tests, use std::time
        use std::time::{SystemTime, UNIX_EPOCH};
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as f64)
            .unwrap_or(0.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_initial_state_is_closed() {
        let cb = CircuitBreaker::new(5, 2, 30000.0);
        assert_eq!(cb.state, CircuitState::Closed);
    }

    #[test]
    fn test_allow_when_closed() {
        let mut cb = CircuitBreaker::new(5, 2, 30000.0);
        assert!(cb.allow());
    }

    #[test]
    fn test_opens_after_threshold() {
        let mut cb = CircuitBreaker::new(3, 2, 30000.0);
        for _ in 0..3 {
            cb.record_failure();
        }
        assert_eq!(cb.state, CircuitState::Open);
        assert!(!cb.allow());
    }

    #[test]
    fn test_success_resets_failure_count() {
        let mut cb = CircuitBreaker::new(3, 2, 30000.0);
        cb.record_failure();
        cb.record_failure();
        cb.record_success();
        assert_eq!(cb.failure_count, 0);
        assert_eq!(cb.state, CircuitState::Closed);
    }

    #[test]
    fn test_half_open_to_closed() {
        let mut cb = CircuitBreaker::new(3, 2, 1000.0);
        // Open the circuit
        for _ in 0..3 {
            cb.record_failure();
        }
        assert_eq!(cb.state, CircuitState::Open);

        // Simulate timeout by backdating last_failure_at
        cb.last_failure_at = now_ms() - 2000.0;

        // Should transition to half-open on allow (timeout elapsed)
        assert!(cb.allow());
        assert_eq!(cb.state, CircuitState::HalfOpen);

        // Two successes should close it
        cb.record_success();
        cb.record_success();
        assert_eq!(cb.state, CircuitState::Closed);
    }

    #[test]
    fn test_half_open_failure_reopens() {
        let mut cb = CircuitBreaker::new(3, 2, 1000.0);
        for _ in 0..3 {
            cb.record_failure();
        }
        // Simulate timeout
        cb.last_failure_at = now_ms() - 2000.0;

        assert!(cb.allow()); // transitions to half-open
        assert_eq!(cb.state, CircuitState::HalfOpen);

        cb.record_failure();
        assert_eq!(cb.state, CircuitState::Open);
    }

    #[test]
    fn test_reset() {
        let mut cb = CircuitBreaker::new(3, 2, 30000.0);
        for _ in 0..3 {
            cb.record_failure();
        }
        assert_eq!(cb.state, CircuitState::Open);

        cb.reset();
        assert_eq!(cb.state, CircuitState::Closed);
        assert_eq!(cb.failure_count, 0);
    }

    #[test]
    fn test_default_thresholds_on_zero() {
        let cb = CircuitBreaker::new(0, 0, 0.0);
        assert_eq!(cb.failure_threshold, 5);
        assert_eq!(cb.success_threshold, 2);
        assert_eq!(cb.open_timeout_ms, 30_000.0);
    }

    #[test]
    fn test_half_open_max_requests() {
        let mut cb = CircuitBreaker::new(1, 2, 1000.0);
        cb.record_failure(); // Open
                             // Simulate timeout
        cb.last_failure_at = now_ms() - 2000.0;
        assert!(cb.allow()); // HalfOpen, count=1
        assert!(cb.allow()); // count=2
        assert!(cb.allow()); // count=3
        assert!(!cb.allow()); // Blocked — max 3 probes
    }

    // ── Additional coverage ──────────────────────────────────────────

    #[test]
    fn test_state_getter_auto_transitions_open_to_half_open() {
        let mut cb = CircuitBreaker::new(2, 1, 1000.0);
        cb.record_failure();
        cb.record_failure();
        assert_eq!(cb.state, CircuitState::Open);

        // Simulate timeout elapsed
        cb.last_failure_at = now_ms() - 2000.0;

        // state() getter should auto-transition
        assert_eq!(cb.state(), CircuitState::HalfOpen);
    }

    #[test]
    fn test_state_getter_stays_open_before_timeout() {
        let mut cb = CircuitBreaker::new(2, 1, 60_000.0);
        cb.record_failure();
        cb.record_failure();
        // Timeout hasn't elapsed
        assert_eq!(cb.state(), CircuitState::Open);
    }

    #[test]
    fn test_success_count_getter() {
        let mut cb = CircuitBreaker::new(2, 3, 1000.0);
        assert_eq!(cb.success_count(), 0);

        // Open the circuit
        cb.record_failure();
        cb.record_failure();
        cb.last_failure_at = now_ms() - 2000.0;
        assert!(cb.allow()); // HalfOpen

        cb.record_success();
        assert_eq!(cb.success_count(), 1);
        cb.record_success();
        assert_eq!(cb.success_count(), 2);
    }

    #[test]
    fn test_failure_count_getter() {
        let mut cb = CircuitBreaker::new(5, 2, 30000.0);
        assert_eq!(cb.failure_count(), 0);
        cb.record_failure();
        assert_eq!(cb.failure_count(), 1);
        cb.record_failure();
        assert_eq!(cb.failure_count(), 2);
    }

    #[test]
    fn test_record_success_in_open_state_is_noop() {
        let mut cb = CircuitBreaker::new(2, 1, 60_000.0);
        cb.record_failure();
        cb.record_failure();
        assert_eq!(cb.state, CircuitState::Open);

        cb.record_success();
        // Should still be open (success in Open is ignored)
        assert_eq!(cb.state, CircuitState::Open);
    }

    #[test]
    fn test_record_failure_in_open_state_is_noop() {
        let mut cb = CircuitBreaker::new(2, 1, 60_000.0);
        cb.record_failure();
        cb.record_failure();
        assert_eq!(cb.state, CircuitState::Open);

        cb.record_failure();
        // Still open, no further state change
        assert_eq!(cb.state, CircuitState::Open);
    }

    #[test]
    fn test_transition_idempotent_same_state() {
        let mut cb = CircuitBreaker::new(5, 2, 30000.0);
        assert_eq!(cb.state, CircuitState::Closed);
        cb.reset(); // Closed -> Closed (no-op)
        assert_eq!(cb.state, CircuitState::Closed);
    }

    #[test]
    fn test_counters_reset_on_transition() {
        let mut cb = CircuitBreaker::new(3, 2, 1000.0);
        cb.record_failure();
        cb.record_failure();
        assert_eq!(cb.failure_count(), 2);

        cb.record_failure(); // Transitions to Open
                             // After transition, counters should be reset
        assert_eq!(cb.failure_count(), 0);
        assert_eq!(cb.success_count(), 0);
    }

    #[test]
    fn test_full_cycle_closed_open_halfopen_closed() {
        let mut cb = CircuitBreaker::new(2, 2, 1000.0);

        // Closed -> Open
        cb.record_failure();
        cb.record_failure();
        assert_eq!(cb.state, CircuitState::Open);

        // Open -> HalfOpen (via timeout)
        cb.last_failure_at = now_ms() - 2000.0;
        assert!(cb.allow());
        assert_eq!(cb.state, CircuitState::HalfOpen);

        // HalfOpen -> Closed (via success threshold)
        cb.record_success();
        assert_eq!(cb.state, CircuitState::HalfOpen);
        cb.record_success();
        assert_eq!(cb.state, CircuitState::Closed);

        // Should be fully functional again
        assert!(cb.allow());
        assert_eq!(cb.failure_count(), 0);
    }

    #[test]
    fn test_half_open_failure_then_full_recovery() {
        let mut cb = CircuitBreaker::new(2, 1, 1000.0);

        // Open the circuit
        cb.record_failure();
        cb.record_failure();

        // First recovery attempt fails
        cb.last_failure_at = now_ms() - 2000.0;
        assert!(cb.allow()); // HalfOpen
        cb.record_failure(); // Back to Open

        // Second recovery attempt succeeds
        cb.last_failure_at = now_ms() - 2000.0;
        assert!(cb.allow()); // HalfOpen again
        cb.record_success();
        assert_eq!(cb.state, CircuitState::Closed);
    }

    #[test]
    fn test_single_failure_threshold() {
        let mut cb = CircuitBreaker::new(1, 1, 1000.0);
        cb.record_failure();
        assert_eq!(cb.state, CircuitState::Open);
    }

    #[test]
    fn test_circuit_state_debug_format() {
        assert_eq!(format!("{:?}", CircuitState::Closed), "Closed");
        assert_eq!(format!("{:?}", CircuitState::Open), "Open");
        assert_eq!(format!("{:?}", CircuitState::HalfOpen), "HalfOpen");
    }

    #[test]
    fn test_circuit_state_clone_copy() {
        let state = CircuitState::HalfOpen;
        let copied = state;
        let cloned = state.clone();
        assert_eq!(state, copied);
        assert_eq!(state, cloned);
    }
}
