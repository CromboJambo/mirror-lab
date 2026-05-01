// Example Kernel Implementations
// These demonstrate how to create custom Mirror Kernels

use crate::{MirrorKernel, MirrorTag, Reflection};
use std::collections::HashSet;

/// Empathic Mirror Kernel
/// Combines events and adds empathic reflection
pub struct EmpathicMirror;

impl MirrorKernel for EmpathicMirror {
    fn name(&self) -> &str {
        "empathic_mirror"
    }

    fn transform(&self, events: &[crate::MirrorEvent]) -> Option<Reflection> {
        let combined = events
            .iter()
            .map(|e| e.content.clone())
            .collect::<Vec<_>>()
            .join(" ");

        Some(Reflection {
            new_content: format!("Empathic reflection: {}", combined),
            new_tags: vec![MirrorTag::Reflect, MirrorTag::EmpathicHigh],
            timestamp: chrono::Utc::now(),
        })
    }

    fn required_tags(&self) -> Vec<MirrorTag> {
        vec![MirrorTag::Reflect]
    }
}

/// Challenge Mirror Kernel
/// Transforms events into challenging questions
pub struct ChallengeMirror;

impl MirrorKernel for ChallengeMirror {
    fn name(&self) -> &str {
        "challenge_mirror"
    }

    fn transform(&self, events: &[crate::MirrorEvent]) -> Option<Reflection> {
        let challenging = events
            .iter()
            .map(|e| format!("Challenge: {}", e.content))
            .collect::<Vec<_>>()
            .join("; ");

        Some(Reflection {
            new_content: format!("Critical challenge: {}", challenging),
            new_tags: vec![MirrorTag::Challenge, MirrorTag::Reflect],
            timestamp: chrono::Utc::now(),
        })
    }

    fn required_tags(&self) -> Vec<MirrorTag> {
        vec![MirrorTag::Challenge]
    }
}

/// Compress Mirror Kernel
/// Shortens event content
pub struct CompressMirror;

impl MirrorKernel for CompressMirror {
    fn name(&self) -> &str {
        "compress_mirror"
    }

    fn transform(&self, events: &[crate::MirrorEvent]) -> Option<Reflection> {
        let compressed = events
            .iter()
            .map(|e| e.content.chars().take(50).collect::<String>())
            .collect::<Vec<_>>()
            .join(" ");

        Some(Reflection {
            new_content: format!("Compressed: {}", compressed),
            new_tags: vec![MirrorTag::Compress, MirrorTag::Reflect],
            timestamp: chrono::Utc::now(),
        })
    }

    fn required_tags(&self) -> Vec<MirrorTag> {
        vec![MirrorTag::Compress]
    }
}

/// Expand Mirror Kernel
/// Expands event content with additional context
pub struct ExpandMirror;

impl MirrorKernel for ExpandMirror {
    fn name(&self) -> &str {
        "expand_mirror"
    }

    fn transform(&self, events: &[crate::MirrorEvent]) -> Option<Reflection> {
        let expanded = events
            .iter()
            .map(|e| format!("{} (expanded)", e.content))
            .collect::<Vec<_>>()
            .join("\n");

        Some(Reflection {
            new_content: format!("Expanded view:\n{}", expanded),
            new_tags: vec![MirrorTag::Expand, MirrorTag::Reflect],
            timestamp: chrono::Utc::now(),
        })
    }

    fn required_tags(&self) -> Vec<MirrorTag> {
        vec![MirrorTag::Expand]
    }
}

/// Delusion Compiler Kernel
/// Compiles high-entropy stream-of-consciousness input into deterministic,
/// indexable units. The constraint "I'm not capable of holding it all" is
/// the design spec — this kernel externalizes what the human can't retain.
///
/// Every output carries its own doubt: assumptions, failure points, staleness,
/// and missed signals. Raw events are never modified; the compiler only produces
/// reflections referencing raw data.
pub struct DelusionCompiler;

impl MirrorKernel for DelusionCompiler {
    fn name(&self) -> &str {
        "delusion_compiler"
    }

    fn transform(&self, events: &[crate::MirrorEvent]) -> Option<Reflection> {
        if events.is_empty() {
            return None;
        }

        let mut claims = Vec::new();
        let mut all_assumptions = HashSet::new();
        let mut all_failure_points = HashSet::new();
        let all_missed: HashSet<String> = HashSet::new();
        let mut total_confidence = 0u32;

        for event in events {
            let (unit_claims, unit_assumptions, unit_confidence) = Self::compile_event(event);

            for claim in unit_claims {
                claims.push(claim);
            }
            for assumption in unit_assumptions {
                all_assumptions.insert(assumption);
            }
            total_confidence += unit_confidence;
        }

        // Structural assumption: compilation assumes the input was coherent enough
        // to extract signals. If the input was pure noise, this is a lie.
        all_assumptions.insert("Input contained extractable signal above noise floor".to_string());

        // Failure point: any compilation loses entropy. The loss is the lie.
        all_failure_points
            .insert("Structured output loses entropy present in raw input".to_string());

        let avg_confidence = if !events.is_empty() {
            total_confidence / events.len() as u32
        } else {
            0
        };

        let reflection_content = format!(
            "COMPILED_UNITS|claims={}|confidence={}|assumptions={}|failure_points={}|missed={}",
            claims.len(),
            avg_confidence,
            all_assumptions.len(),
            all_failure_points.len(),
            all_missed.len()
        );

        Some(Reflection {
            new_content: reflection_content,
            new_tags: vec![MirrorTag::Compile, MirrorTag::Reflect],
            timestamp: chrono::Utc::now(),
        })
    }

    fn required_tags(&self) -> Vec<MirrorTag> {
        vec![MirrorTag::Compile]
    }
}

impl DelusionCompiler {
    /// Compile a single event into structured claims with confidence scoring.
    /// Returns (claims, assumptions, confidence_score).
    fn compile_event(event: &crate::MirrorEvent) -> (Vec<String>, Vec<String>, u32) {
        let content = &event.content;
        let mut claims = Vec::new();
        let mut assumptions = Vec::new();
        let mut confidence = 100u32;

        // Extract sentence-level claims from raw content
        // Simple heuristic: split on sentence boundaries
        let sentences: Vec<&str> = content
            .split(|c: char| ['.', '!', '?', '\n'].contains(&c))
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .collect();

        for sentence in &sentences {
            // Penalize confidence for vague language
            let lower = sentence.to_lowercase();
            if lower.contains("maybe") || lower.contains("perhaps") || lower.contains("sort of") {
                confidence = confidence.saturating_sub(20);
                assumptions.push(format!(
                    "Claim contains hedged language: '{}'",
                    sentence.chars().take(80).collect::<String>()
                ));
            }

            // Penalize for emotional content (reduces determinism)
            if lower.contains("feel") || lower.contains("think") || lower.contains("believe") {
                confidence = confidence.saturating_sub(15);
                assumptions.push(format!(
                    "Claim contains subjective framing: '{}'",
                    sentence.chars().take(80).collect::<String>()
                ));
            }

            // Penalize for superlatives (absolute claims are brittle)
            if lower.contains("always")
                || lower.contains("never")
                || lower.contains("everything")
                || lower.contains("nothing")
            {
                confidence = confidence.saturating_sub(25);
                assumptions.push(format!(
                    "Claim contains absolute language: '{}'",
                    sentence.chars().take(80).collect::<String>()
                ));
            }

            claims.push(sentence.to_string());
        }

        // Base assumption: the source is truthful about what it observed
        assumptions.push(format!(
            "Source '{}' reports observations in good faith",
            event.source
        ));

        // Base assumption: temporal relevance
        assumptions.push("Content is temporally relevant to current context".to_string());

        (claims, assumptions, confidence.max(5))
    }
}
