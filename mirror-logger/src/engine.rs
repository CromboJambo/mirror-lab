use crate::tri::Tri;

/// Core engine layer with BitNet-inspired simple state transitions
#[allow(dead_code)]
pub struct Engine {
    // Future: weights, patterns, etc.
}

#[allow(dead_code)]
impl Engine {
    /// Create a new engine instance
    pub fn new() -> Self {
        Self {}
    }

    /// Apply state transformation with signal
    ///
    /// This is the core function for scoring decisions and pattern detection
    pub fn apply(state: Tri, signal: f32) -> f32 {
        match state {
            Tri::Pos => signal,
            Tri::Neg => -signal,
            Tri::Zero => 0.0,
        }
    }

    /// Calculate decision score based on state and confidence
    pub fn score(state: Tri, confidence: f32) -> f32 {
        let signal = (confidence * 2.0) - 1.0; // Normalize confidence to -1.0 to 1.0
        Self::apply(state, signal)
    }

    /// Check if a state is "finalized" (Pos or Neg, not Hold)
    pub fn is_finalized(state: Tri) -> bool {
        matches!(state, Tri::Pos | Tri::Neg)
    }

    /// Check if a state is "pending" (Hold)
    pub fn is_pending(state: Tri) -> bool {
        state == Tri::Zero
    }

    /// Get state direction for visualization
    pub fn direction(state: Tri) -> &'static str {
        match state {
            Tri::Pos => "↑",
            Tri::Neg => "↓",
            Tri::Zero => "→",
        }
    }

    /// Calculate momentum (change from previous state)
    pub fn momentum(current: Tri, previous: Tri) -> Tri {
        // Positive momentum: Pos → Pos
        if current == Tri::Pos && previous == Tri::Pos {
            return Tri::Pos;
        }
        // Negative momentum: Neg → Neg
        if current == Tri::Neg && previous == Tri::Neg {
            return Tri::Neg;
        }
        // Zero momentum: anything to Zero
        Tri::Zero
    }
}

impl Default for Engine {
    fn default() -> Self {
        Self::new()
    }
}

/// Pattern detection hooks
#[allow(dead_code)]
pub trait PatternHooks {
    /// Detect repeated failures on same tag
    fn detect_repeated_failures(entries: &[crate::entry::MirrorEntry]) -> Vec<(String, Vec<u64>)> {
        let mut failures: std::collections::HashMap<String, Vec<u64>> =
            std::collections::HashMap::new();

        for entry in entries {
            if entry.state == crate::tri::Tri::Neg {
                for tag in &entry.tags {
                    failures.entry(tag.clone()).or_default().push(entry.id);
                }
            }
        }

        failures
            .into_iter()
            .filter(|(_, ids)| ids.len() >= 2)
            .collect()
    }

    /// Detect Hold → Pos transitions
    fn detect_hold_to_pos(entries: &[crate::entry::MirrorEntry]) -> Vec<crate::entry::MirrorEntry> {
        entries
            .iter()
            .filter(|e| e.state == crate::tri::Tri::Pos && e.parent.is_some())
            .cloned()
            .collect()
    }

    /// Detect repeated failures with reasoning
    fn detect_repeated_failure_patterns(entries: &[crate::entry::MirrorEntry]) -> Vec<String> {
        let mut patterns = Vec::new();

        // Group by input
        let mut inputs: std::collections::HashMap<String, Vec<&crate::entry::MirrorEntry>> =
            std::collections::HashMap::new();

        for entry in entries {
            if entry.state == crate::tri::Tri::Neg {
                inputs.entry(entry.input.clone()).or_default().push(entry);
            }
        }

        for (input, entries) in inputs {
            if entries.len() >= 2 {
                patterns.push(format!(
                    "Repeated failure: '{}' ({} times)",
                    input,
                    entries.len()
                ));
            }
        }

        patterns
    }
}

impl PatternHooks for Engine {}
