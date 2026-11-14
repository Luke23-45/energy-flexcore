// P7.18 — The worker-supervision test suite (ledger P7.18 step 4):
//         timeout fires with an injected clock, the retry/backoff
//         schedule is deterministic, a crash marks FAILED and degrades
//         the pool, backpressure refuses with a named error, recovery
//         requires probe success AND a policy record, and every step
//         leaves an audit record.
//
// ══════════════════════════════════════════════════════════════════════════
// HONESTY HEADER
//
// WRITTEN, NEVER RUN. No cargo command (build, test, clippy, fmt) was
// executed in this loop; the suite has never compiled or passed. It is
// written against the public surface of `flexcore_worker::supervision`
// and the port fake `flexcore_ports::fakes::FixedClock` (both existing,
// grep-verified crates/types) and is expected to compile and pass at the
// first human build (`cargo test -p flexcore-worker --test
// supervision_p7_18`). Everything asserted here is a design statement,
// not a tool-verified fact, until that run is recorded.
//
// The suite is PURE: no database, no broker, no network, no tokio — the
// supervisor module is std-only logic over the injected P1.07 Clock, so
// every scenario below is fully deterministic.
// ══════════════════════════════════════════════════════════════════════════

use flexcore_ports::fakes::FixedClock;
use flexcore_worker::supervision::{
    CallKind, PoolStatus, RetryDecision, SupervisionAuditKind, SupervisionAuditSink,
    SupervisionPolicy, SupervisionError, SupervisedWorkerState, WorkerFailureKind, WorkerPool,
};

// The scenario clock start (arbitrary, fixed: 2026-11-13T00:00:00Z).
const T0_MS: i64 = 1_764_288_000_000;

fn idem(key: &str) -> CallKind {
    CallKind::Idempotent {
        idempotency_key: key.to_string(),
    }
}

fn non_idem() -> CallKind {
    CallKind::NonIdempotent
}

fn policy_with(cap: u32) -> SupervisionPolicy {
    SupervisionPolicy {
        inflight_cap: cap,
        ..SupervisionPolicy::default()
    }
}

/// Registers and readies `count` workers on the pool.
fn ready_pool(
    pool: &mut WorkerPool,
    audit: &dyn SupervisionAuditSink,
    clock: &FixedClock,
    count: usize,
) {
    for i in 0..count {
        let id = format!("worker-{i}");
        pool.register(&id, audit, clock).expect("register");
        pool.mark_ready(&id, audit, clock).expect("mark_ready");
    }
}

// ---------------------------------------------------------------------------
// Timeout fires (injected clock)
// ---------------------------------------------------------------------------

#[test]
fn timeout_fires_at_the_deadline_on_the_injected_clock() {
    let audit = flexcore_worker::supervision::MemorySupervisionAuditSink::new();
    let clock = FixedClock::at_unix_millis(T0_MS);
    let mut pool = WorkerPool::new(policy_with(2)).expect("valid policy");
    ready_pool(&mut pool, &audit, &clock, 1);

    let deadline = pool
        .acquire("worker-0", "req-1", &idem("idem-1"), &audit, &clock)
        .expect("acquire under the cap");
    assert_eq!(deadline.started_at_ms(), T0_MS);
    assert_eq!(
        deadline.deadline_ms(),
        T0_MS + pool.policy().call_timeout_ms as i64
    );

    // Just before the deadline: not expired.
    clock.advance_millis(pool.policy().call_timeout_ms as i64 - 1);
    assert!(!deadline.expired_at(clock.unix_millis()));

    // AT the deadline: expired (fail-closed boundary: the deadline
    // instant itself is already too late).
    clock.advance_millis(1);
    assert!(deadline.expired_at(clock.unix_millis()));
}

#[test]
fn a_recorded_timeout_is_a_failure_with_diagnostics_not_a_hang() {
    let audit = flexcore_worker::supervision::MemorySupervisionAuditSink::new();
    let clock = FixedClock::at_unix_millis(T0_MS);
    let mut pool = WorkerPool::new(policy_with(2)).expect("valid policy");
    ready_pool(&mut pool, &audit, &clock, 1);

    let deadline = pool
        .acquire("worker-0", "req-1", &idem("idem-1"), &audit, &clock)
        .expect("acquire");
    clock.advance_millis(pool.policy().call_timeout_ms as i64 + 1);

    // Idempotent call, attempt 0: the decision is a bounded retry.
    let decision = pool
        .record_timeout("worker-0", "req-1", &idem("idem-1"), 0, &deadline, &audit, &clock)
        .expect("timeout recorded");
    match decision {
        RetryDecision::RetryAfter { delay_ms, next_attempt } => {
            assert_eq!(next_attempt, 1);
            // Exponential backoff base (attempt 0 -> base) plus seeded
            // jitter, both bounded by the policy.
            assert!(delay_ms >= pool.policy().backoff_base_ms);
            assert!(delay_ms <= pool.policy().backoff_cap_ms + pool.policy().jitter_cap_ms);
        }
        other => panic!("expected RetryAfter, got {other:?}"),
    }
    // The failure is audited with diagnostics.
    let events = audit.records();
    assert!(
        events
            .iter()
            .any(|e| e.kind == SupervisionAuditKind::FailureRecorded
                && e.request_id == "req-1"
                && e.detail.contains("TIMEOUT")),
        "timeout must produce a FAILURE_RECORDED audit event: {events:?}"
    );
    // The in-flight slot was released with the recorded failure.
    assert_eq!(pool.ready_worker_count(), 1);
}

// ---------------------------------------------------------------------------
// Retry/backoff schedule is deterministic (P7.42 contract)
// ---------------------------------------------------------------------------

#[test]
fn retry_backoff_schedule_is_deterministic_for_a_seed() {
    // Two identically-seeded pools produce the identical schedule.
    let audit = flexcore_worker::supervision::MemorySupervisionAuditSink::new();
    let clock = FixedClock::at_unix_millis(T0_MS);
    let mut first = WorkerPool::new(policy_with(4)).expect("valid policy");
    let mut second = WorkerPool::new(policy_with(4)).expect("valid policy");
    ready_pool(&mut first, &audit, &clock, 1);
    ready_pool(&mut second, &audit, &clock, 1);

    let mut schedule_first = Vec::new();
    let mut schedule_second = Vec::new();
    let mut deadline_first = first
        .acquire("worker-0", "req-1", &idem("idem-1"), &audit, &clock)
        .expect("acquire");
    let mut deadline_second = second
        .acquire("worker-0", "req-1", &idem("idem-1"), &audit, &clock)
        .expect("acquire");
    for attempt in 0..first.policy().max_retries {
        let d1 = first
            .record_timeout("worker-0", "req-1", &idem("idem-1"), attempt, &deadline_first, &audit, &clock)
            .expect("failure recorded");
        let d2 = second
            .record_timeout("worker-0", "req-1", &idem("idem-1"), attempt, &deadline_second, &audit, &clock)
            .expect("failure recorded");
        assert_eq!(d1, d2, "same seed must give the same schedule");
        match d1 {
            RetryDecision::RetryAfter { delay_ms, next_attempt } => {
                schedule_first.push((delay_ms, next_attempt));
                schedule_second.push((delay_ms, next_attempt));
                // The next attempt needs a fresh call to time out.
                clock.advance_millis(delay_ms as i64);
                deadline_first = first
                    .acquire("worker-0", "req-1", &idem("idem-1"), &audit, &clock)
                    .expect("re-acquire for next attempt");
                deadline_second = second
                    .acquire("worker-0", "req-1", &idem("idem-1"), &audit, &clock)
                    .expect("re-acquire for next attempt");
            }
            other => panic!("expected RetryAfter at attempt {attempt}, got {other:?}"),
        }
    }
    assert_eq!(schedule_first, schedule_second);
    assert_eq!(schedule_first.len(), first.policy().max_retries as usize);
    // The re-acquired deadlines after the final attempt are deliberately
    // unused: the schedule under test is complete.
    let _ = (deadline_first, deadline_second);
}

#[test]
fn retries_are_bounded_and_then_refused() {
    let audit = flexcore_worker::supervision::MemorySupervisionAuditSink::new();
    let clock = FixedClock::at_unix_millis(T0_MS);
    let mut pool = WorkerPool::new(SupervisionPolicy {
        max_retries: 2,
        failure_threshold: 100, // keep the worker alive across the attempts
        ..SupervisionPolicy::default()
    })
    .expect("valid policy");
    ready_pool(&mut pool, &audit, &clock, 1);

    let kinds = [
        WorkerFailureKind::Timeout,
        WorkerFailureKind::ContractViolation,
        WorkerFailureKind::Timeout,
        WorkerFailureKind::Timeout,
    ];
    let mut decision = None;
    for (attempt, kind) in kinds.iter().enumerate() {
        let deadline = pool
            .acquire("worker-0", "req-1", &idem("idem-1"), &audit, &clock)
            .expect("acquire per attempt");
        decision = Some(
            pool.record_failure(
                "worker-0",
                "req-1",
                *kind,
                &idem("idem-1"),
                attempt as u32,
                format!("attempt {attempt} of {kind:?}"),
                &audit,
                &clock,
            )
            .expect("failure recorded"),
        );
    }
    // attempt 2 == max_retries (2 retries already granted at attempts
    // 0 and 1) -> attempt 2 is the last retry; attempt 3 is refused.
    assert!(matches!(
        decision,
        Some(RetryDecision::NoRetry { reason: "retries exhausted" })
    ));
    let events = audit.records();
    assert!(events
        .iter()
        .any(|e| e.kind == SupervisionAuditKind::RetryRefused));
}

#[test]
fn non_idempotent_calls_are_never_retried() {
    let audit = flexcore_worker::supervision::MemorySupervisionAuditSink::new();
    let clock = FixedClock::at_unix_millis(T0_MS);
    // A worker-side "retryable" hint does not exist here by design; the
    // rule is structural: NonIdempotent never yields RetryAfter.
    let mut pool = WorkerPool::new(SupervisionPolicy {
        failure_threshold: 100,
        ..SupervisionPolicy::default()
    })
    .expect("valid policy");
    ready_pool(&mut pool, &audit, &clock, 1);
    let deadline = pool
        .acquire("worker-0", "req-1", &non_idem(), &audit, &clock)
        .expect("acquire");
    let decision = pool
        .record_timeout("worker-0", "req-1", &non_idem(), 0, &deadline, &audit, &clock)
        .expect("recorded");
    assert!(matches!(
        decision,
        RetryDecision::NoRetry { reason: "non-idempotent call" }
    ));
    assert!(audit
        .records()
        .iter()
        .any(|e| e.kind == SupervisionAuditKind::RetryRefused));
}

#[test]
fn an_idempotent_call_without_a_key_is_refused_fail_closed() {
    let audit = flexcore_worker::supervision::MemorySupervisionAuditSink::new();
    let clock = FixedClock::at_unix_millis(T0_MS);
    let mut pool = WorkerPool::new(policy_with(2)).expect("valid policy");
    ready_pool(&mut pool, &audit, &clock, 1);
    let result = pool.acquire(
        "worker-0",
        "req-1",
        &CallKind::Idempotent {
            idempotency_key: String::new(),
        },
        &audit,
        &clock,
    );
    assert!(matches!(
        result,
        Err(SupervisionError::MissingIdempotencyKey { .. })
    ));
}

// ---------------------------------------------------------------------------
// Crash isolation: FAILED + degraded pool, no silent restart
// ---------------------------------------------------------------------------

#[test]
fn crash_marks_failed_and_degrades_the_pool() {
    let audit = flexcore_worker::supervision::MemorySupervisionAuditSink::new();
    let clock = FixedClock::at_unix_millis(T0_MS);
    let mut pool = WorkerPool::new(policy_with(4)).expect("valid policy");
    ready_pool(&mut pool, &audit, &clock, 2);
    assert!(!pool.status().is_degraded());

    pool.record_crash("worker-0", "killed mid-request", &audit, &clock)
        .expect("crash recorded");

    // The pool reports DEGRADED, naming the failed worker.
    let status = pool.status();
    assert!(status.is_degraded());
    match &status {
        PoolStatus::Degraded { failed_workers, .. } => {
            assert_eq!(failed_workers, &vec!["worker-0".to_string()]);
        }
        other => panic!("expected Degraded, got {other:?}"),
    }
    assert_eq!(pool.crashes_recorded(), 1);
    // The crashed worker is out of the ready set.
    assert_eq!(pool.ready_worker_count(), 1);
    let result = pool.acquire("worker-0", "req-9", &idem("idem-9"), &audit, &clock);
    assert!(matches!(
        result,
        Err(SupervisionError::WorkerNotReady { state: SupervisedWorkerState::Failed, .. })
    ));
    // The crash is audited.
    assert!(audit.records().iter().any(|e| e.kind == SupervisionAuditKind::WorkerFailed));
}

// ---------------------------------------------------------------------------
// Backpressure: bounded in-flight, refusal is named and audited
// ---------------------------------------------------------------------------

#[test]
fn backpressure_refuses_beyond_the_cap_and_never_queues() {
    let audit = flexcore_worker::supervision::MemorySupervisionAuditSink::new();
    let clock = FixedClock::at_unix_millis(T0_MS);
    let mut pool = WorkerPool::new(policy_with(2)).expect("valid policy");
    ready_pool(&mut pool, &audit, &clock, 1);

    pool.acquire("worker-0", "req-1", &idem("k1"), &audit, &clock)
        .expect("first in flight");
    pool.acquire("worker-0", "req-2", &idem("k2"), &audit, &clock)
        .expect("second in flight");
    let refused = pool.acquire("worker-0", "req-3", &idem("k3"), &audit, &clock);
    match refused {
        Err(SupervisionError::BackpressureRefused { inflight, cap }) => {
            assert_eq!((inflight, cap), (2, 2));
        }
        other => panic!("expected BackpressureRefused, got {other:?}"),
    }
    assert_eq!(pool.backpressure_refusals(), 1);
    // The refusal is audited with the depth — never silently dropped.
    assert!(audit.records().iter().any(
        |e| e.kind == SupervisionAuditKind::BackpressureRefused && e.request_id == "req-3"
    ));
    // Completing work re-opens the cap (no leaked slot).
    pool.record_completed("worker-0", "req-1", &audit, &clock)
        .expect("completion recorded");
    pool.acquire("worker-0", "req-3", &idem("k3"), &audit, &clock)
        .expect("cap freed by completion");
}

// ---------------------------------------------------------------------------
// Recovery: probe success + policy record; never a silent restart
// ---------------------------------------------------------------------------

#[test]
fn recovery_requires_probe_success_and_a_policy_record() {
    let audit = flexcore_worker::supervision::MemorySupervisionAuditSink::new();
    let clock = FixedClock::at_unix_millis(T0_MS);
    let mut pool = WorkerPool::new(policy_with(4)).expect("valid policy");
    ready_pool(&mut pool, &audit, &clock, 1);
    pool.record_crash("worker-0", "process died", &audit, &clock)
        .expect("crash recorded");

    // A probe success WITHOUT a policy record never restarts the worker.
    let unauthorized = pool.recover_on_probe_success("worker-0", "", &audit, &clock);
    assert!(matches!(
        unauthorized,
        Err(SupervisionError::RecoveryNotAuthorized { .. })
    ));
    assert_eq!(pool.ready_worker_count(), 0);

    // Probe success WITH the authorizing policy record: FAILED ->
    // STARTING (re-initialization), then READY only via mark_ready.
    pool.recover_on_probe_success("worker-0", "supervision-policy/restart-p7-18", &audit, &clock)
        .expect("authorized recovery");
    assert_eq!(pool.ready_worker_count(), 0); // not yet ready
    pool.mark_ready("worker-0", &audit, &clock)
        .expect("ready after re-initialization");
    assert_eq!(pool.ready_worker_count(), 1);
    assert!(!pool.status().is_degraded());
    pool.acquire("worker-0", "req-10", &idem("k10"), &audit, &clock)
        .expect("recovered worker serves again");
    // The recovery path is audited end to end.
    let events = audit.records();
    assert!(events
        .iter()
        .any(|e| e.kind == SupervisionAuditKind::ProbeRecovered));
    assert!(events
        .iter()
        .any(|e| e.kind == SupervisionAuditKind::WorkerReinstated));
}

// ---------------------------------------------------------------------------
// Failure threshold: consecutive failures mark FAILED
// ---------------------------------------------------------------------------

#[test]
fn consecutive_timeouts_reach_the_threshold_and_fail_the_worker() {
    let audit = flexcore_worker::supervision::MemorySupervisionAuditSink::new();
    let clock = FixedClock::at_unix_millis(T0_MS);
    let mut pool = WorkerPool::new(SupervisionPolicy {
        failure_threshold: 2,
        ..SupervisionPolicy::default()
    })
    .expect("valid policy");
    ready_pool(&mut pool, &audit, &clock, 1);

    for attempt in 0..2u32 {
        let deadline = pool
            .acquire("worker-0", "req-1", &idem("idem-1"), &audit, &clock)
            .expect("acquire");
        pool.record_timeout("worker-0", "req-1", &idem("idem-1"), attempt, &deadline, &audit, &clock)
            .expect("timeout recorded");
    }
    assert!(pool.status().is_degraded());
    assert_eq!(pool.ready_worker_count(), 0);
}

// ---------------------------------------------------------------------------
// Lifecycle consistency + audit trail shape
// ---------------------------------------------------------------------------

#[test]
fn drain_path_follows_the_kernel_vocabulary() {
    let audit = flexcore_worker::supervision::MemorySupervisionAuditSink::new();
    let clock = FixedClock::at_unix_millis(T0_MS);
    let mut pool = WorkerPool::new(policy_with(2)).expect("valid policy");
    ready_pool(&mut pool, &audit, &clock, 1);
    pool.drain("worker-0", &audit, &clock).expect("drain");
    assert_eq!(pool.ready_worker_count(), 0);
    pool.finish_drain("worker-0", &audit, &clock).expect("stop");
    // The recorded seam: the state names mirror the kernel lifecycle.
    assert_eq!(
        flexcore_worker::supervision::LIFECYCLE_CONSISTENCY_SEAM,
        "crates/flexcore-kernel/src/lifecycle.rs::LifecycleState (names mirrored; kernel NOT touched)"
    );
}

#[test]
fn every_supervised_step_leaves_an_audit_record_in_order() {
    let audit = flexcore_worker::supervision::MemorySupervisionAuditSink::new();
    let clock = FixedClock::at_unix_millis(T0_MS);
    let mut pool = WorkerPool::new(policy_with(2)).expect("valid policy");
    pool.register("worker-0", &audit, &clock).expect("register");
    pool.mark_ready("worker-0", &audit, &clock).expect("ready");
    let deadline = pool
        .acquire("worker-0", "req-1", &idem("idem-1"), &audit, &clock)
        .expect("acquire");
    clock.advance_millis(pool.policy().call_timeout_ms as i64 + 1);
    pool.record_timeout("worker-0", "req-1", &idem("idem-1"), 0, &deadline, &audit, &clock)
        .expect("timeout recorded");
    pool.record_completed("worker-0", "req-1", &audit, &clock)
        .expect("completion recorded (the retried call's second leg)");
    pool.record_crash("worker-0", "killed", &audit, &clock)
        .expect("crash recorded");

    let kinds: Vec<SupervisionAuditKind> = audit
        .records()
        .iter()
        .map(|e| e.kind)
        .collect();
    assert_eq!(
        kinds,
        vec![
            SupervisionAuditKind::WorkerRegistered,
            SupervisionAuditKind::WorkerReady,
            SupervisionAuditKind::CallAccepted,
            SupervisionAuditKind::FailureRecorded,
            SupervisionAuditKind::RetryScheduled,
            SupervisionAuditKind::CallCompleted,
            SupervisionAuditKind::WorkerFailed,
        ],
        "the audit trail is the ordered evidence of the supervision story"
    );
    // Every event carries the injected-clock instant and is log-safe
    // (identifiers and reasons, never payload bodies).
    for event in audit.records() {
        assert!(event.observed_at_ms >= T0_MS);
        assert!(!event.detail.contains('\n'));
    }
}

#[test]
fn a_broken_audit_sink_refuses_the_supervised_action() {
    struct BrokenSink;
    impl SupervisionAuditSink for BrokenSink {
        fn record(
            &self,
            _event: flexcore_worker::supervision::SupervisionAuditEvent,
        ) -> Result<(), String> {
            Err("sink down".to_string())
        }
    }

    let broken = BrokenSink;
    let clock = FixedClock::at_unix_millis(T0_MS);
    let mut pool = WorkerPool::new(policy_with(2)).expect("valid policy");
    // Audit-or-refuse: registration itself refuses when the sink is down.
    let result = pool.register("worker-0", &broken, &clock);
    assert!(matches!(
        result,
        Err(SupervisionError::AuditUnavailable { .. })
    ));
    assert_eq!(pool.ready_worker_count(), 0);
}
