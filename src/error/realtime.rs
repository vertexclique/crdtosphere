//! Real-time specific error types
//!
//! This module defines error types related to real-time constraints
//! and timing requirements in embedded systems.

/// Real-time error types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RealTimeError {
    /// Deadline missed
    DeadlineMissed {
        /// Expected deadline in CPU cycles
        expected_cycles: u32,
        /// Actual cycles taken
        actual_cycles: u32,
    },
    /// Execution time exceeded budget
    ExecutionTimeExceeded {
        /// Budget in CPU cycles
        budget_cycles: u32,
        /// Actual cycles used
        used_cycles: u32,
    },
    /// Jitter tolerance exceeded
    JitterExceeded {
        /// Maximum allowed jitter in cycles
        max_jitter: u32,
        /// Actual jitter measured
        actual_jitter: u32,
    },
    /// Priority inversion detected
    PriorityInversion {
        /// High priority task ID
        high_priority_task: u8,
        /// Low priority task ID blocking
        blocking_task: u8,
    },
    /// Interrupt latency exceeded
    InterruptLatencyExceeded {
        /// Maximum allowed latency in cycles
        max_latency: u32,
        /// Actual latency measured
        actual_latency: u32,
    },
    /// Context switch overhead exceeded
    ContextSwitchOverhead {
        /// Expected overhead in cycles
        expected_overhead: u32,
        /// Actual overhead measured
        actual_overhead: u32,
    },
    /// Watchdog timeout
    WatchdogTimeout,
    /// Real-time scheduler overrun
    SchedulerOverrun,
    /// Timer resolution insufficient
    TimerResolutionInsufficient,
    /// Clock drift detected
    ClockDrift {
        /// Expected frequency
        expected_freq: u32,
        /// Actual frequency
        actual_freq: u32,
    },
    /// Resource contention timeout
    ResourceContentionTimeout,
    /// Synchronization timeout
    SynchronizationTimeout,
}

/// Real-time priority levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum RealTimePriority {
    /// Background/idle priority
    Background = 0,
    /// Low priority
    Low = 1,
    /// Normal priority
    Normal = 2,
    /// High priority
    High = 3,
    /// Critical priority
    Critical = 4,
    /// Interrupt priority
    Interrupt = 5,
}

/// Real-time scheduling policy
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SchedulingPolicy {
    /// Rate Monotonic Scheduling
    RateMonotonic,
    /// Earliest Deadline First
    EarliestDeadlineFirst,
    /// Fixed Priority Preemptive
    FixedPriorityPreemptive,
    /// Round Robin
    RoundRobin,
    /// Custom scheduling policy
    Custom,
}

/// Real-time task characteristics
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TaskCharacteristics {
    /// Task priority
    pub priority: RealTimePriority,
    /// Period in CPU cycles
    pub period_cycles: u32,
    /// Worst-case execution time in cycles
    pub wcet_cycles: u32,
    /// Deadline in cycles (relative to period start)
    pub deadline_cycles: u32,
    /// Jitter tolerance in cycles
    pub jitter_tolerance: u32,
}

impl TaskCharacteristics {
    /// Creates new task characteristics
    pub const fn new(
        priority: RealTimePriority,
        period_cycles: u32,
        wcet_cycles: u32,
        deadline_cycles: u32,
        jitter_tolerance: u32,
    ) -> Self {
        Self {
            priority,
            period_cycles,
            wcet_cycles,
            deadline_cycles,
            jitter_tolerance,
        }
    }

    /// Returns the utilization factor (WCET / Period)
    pub const fn utilization(&self) -> f32 {
        self.wcet_cycles as f32 / self.period_cycles as f32
    }

    /// Checks if the task is schedulable under Rate Monotonic
    pub const fn is_rm_schedulable(&self) -> bool {
        self.deadline_cycles >= self.wcet_cycles && self.deadline_cycles <= self.period_cycles
    }

    /// Checks if the task meets its deadline constraint
    pub const fn meets_deadline(&self, execution_time: u32) -> bool {
        execution_time <= self.deadline_cycles
    }

    /// Checks if jitter is within tolerance
    pub const fn jitter_acceptable(&self, jitter: u32) -> bool {
        jitter <= self.jitter_tolerance
    }
}

impl RealTimeError {
    /// Returns true if this is a critical real-time error
    pub const fn is_critical(&self) -> bool {
        match self {
            Self::DeadlineMissed { .. }
            | Self::PriorityInversion { .. }
            | Self::WatchdogTimeout
            | Self::SchedulerOverrun => true,
            _ => false,
        }
    }

    /// Returns true if this error is recoverable
    pub const fn is_recoverable(&self) -> bool {
        match self {
            // Non-recoverable critical errors
            Self::WatchdogTimeout | Self::SchedulerOverrun => false,

            // Potentially recoverable errors
            Self::DeadlineMissed { .. }
            | Self::ExecutionTimeExceeded { .. }
            | Self::JitterExceeded { .. }
            | Self::PriorityInversion { .. }
            | Self::InterruptLatencyExceeded { .. }
            | Self::ContextSwitchOverhead { .. }
            | Self::TimerResolutionInsufficient
            | Self::ClockDrift { .. }
            | Self::ResourceContentionTimeout
            | Self::SynchronizationTimeout => true,
        }
    }

    /// Returns the error category
    pub const fn category(&self) -> &'static str {
        match self {
            Self::DeadlineMissed { .. }
            | Self::ExecutionTimeExceeded { .. }
            | Self::JitterExceeded { .. } => "Timing",

            Self::PriorityInversion { .. } | Self::SchedulerOverrun => "Scheduling",

            Self::InterruptLatencyExceeded { .. } | Self::ContextSwitchOverhead { .. } => "System",

            Self::WatchdogTimeout => "Safety",

            Self::TimerResolutionInsufficient | Self::ClockDrift { .. } => "Clock",

            Self::ResourceContentionTimeout | Self::SynchronizationTimeout => "Synchronization",
        }
    }

    /// Returns the severity level
    pub const fn severity(&self) -> u8 {
        match self {
            // Critical (level 4)
            Self::WatchdogTimeout | Self::SchedulerOverrun => 4,

            // High (level 3)
            Self::DeadlineMissed { .. } | Self::PriorityInversion { .. } => 3,

            // Medium (level 2)
            Self::ExecutionTimeExceeded { .. }
            | Self::InterruptLatencyExceeded { .. }
            | Self::JitterExceeded { .. } => 2,

            // Low (level 1)
            Self::ContextSwitchOverhead { .. }
            | Self::TimerResolutionInsufficient
            | Self::ClockDrift { .. }
            | Self::ResourceContentionTimeout
            | Self::SynchronizationTimeout => 1,
        }
    }
}

impl RealTimePriority {
    /// Returns the numeric priority value
    pub const fn numeric_value(&self) -> u8 {
        *self as u8
    }

    /// Checks if this priority is higher than another
    pub const fn is_higher_than(&self, other: &Self) -> bool {
        self.numeric_value() > other.numeric_value()
    }

    /// Checks if this priority can preempt another
    pub const fn can_preempt(&self, other: &Self) -> bool {
        self.is_higher_than(other)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_task_characteristics() {
        let task = TaskCharacteristics::new(
            RealTimePriority::High,
            1000, // 1000 cycle period
            300,  // 300 cycle WCET
            800,  // 800 cycle deadline
            50,   // 50 cycle jitter tolerance
        );

        assert_eq!(task.utilization(), 0.3);
        assert!(task.is_rm_schedulable());
        assert!(task.meets_deadline(300));
        assert!(!task.meets_deadline(900));
        assert!(task.jitter_acceptable(30));
        assert!(!task.jitter_acceptable(60));
    }

    #[test]
    fn test_priority_ordering() {
        assert!(RealTimePriority::Critical > RealTimePriority::High);
        assert!(RealTimePriority::High > RealTimePriority::Normal);
        assert!(RealTimePriority::Normal > RealTimePriority::Low);
        assert!(RealTimePriority::Low > RealTimePriority::Background);

        assert!(RealTimePriority::Critical.is_higher_than(&RealTimePriority::High));
        assert!(RealTimePriority::Critical.can_preempt(&RealTimePriority::Normal));
    }

    #[test]
    fn test_error_classification() {
        let deadline_error = RealTimeError::DeadlineMissed {
            expected_cycles: 1000,
            actual_cycles: 1200,
        };

        let watchdog_error = RealTimeError::WatchdogTimeout;

        assert!(deadline_error.is_critical());
        assert!(deadline_error.is_recoverable());
        assert_eq!(deadline_error.category(), "Timing");
        assert_eq!(deadline_error.severity(), 3);

        assert!(watchdog_error.is_critical());
        assert!(!watchdog_error.is_recoverable());
        assert_eq!(watchdog_error.category(), "Safety");
        assert_eq!(watchdog_error.severity(), 4);
    }

    #[test]
    fn test_error_categories() {
        assert_eq!(
            RealTimeError::DeadlineMissed {
                expected_cycles: 100,
                actual_cycles: 150
            }
            .category(),
            "Timing"
        );
        assert_eq!(
            RealTimeError::PriorityInversion {
                high_priority_task: 1,
                blocking_task: 2
            }
            .category(),
            "Scheduling"
        );
        assert_eq!(
            RealTimeError::InterruptLatencyExceeded {
                max_latency: 50,
                actual_latency: 75
            }
            .category(),
            "System"
        );
        assert_eq!(RealTimeError::WatchdogTimeout.category(), "Safety");
        assert_eq!(
            RealTimeError::ClockDrift {
                expected_freq: 1000,
                actual_freq: 1010
            }
            .category(),
            "Clock"
        );
        assert_eq!(
            RealTimeError::SynchronizationTimeout.category(),
            "Synchronization"
        );
    }

    #[test]
    fn test_severity_levels() {
        assert_eq!(RealTimeError::WatchdogTimeout.severity(), 4);
        assert_eq!(
            RealTimeError::DeadlineMissed {
                expected_cycles: 100,
                actual_cycles: 150
            }
            .severity(),
            3
        );
        assert_eq!(
            RealTimeError::ExecutionTimeExceeded {
                budget_cycles: 100,
                used_cycles: 150
            }
            .severity(),
            2
        );
        assert_eq!(
            RealTimeError::ClockDrift {
                expected_freq: 1000,
                actual_freq: 1010
            }
            .severity(),
            1
        );
    }
}
