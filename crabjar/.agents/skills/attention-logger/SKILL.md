---
name: attention-logger
description: |
  Log, analyze, or inspect the state of the mirror-log attention layer. Use this skill whenever the user wants to check which items are currently "active" in attention, see what items are flagged for decay, view attention statistics (like active/total event ratios), or understand the current importance scores of specific events. Trigger when the user asks "what is in attention?", "check attention stats", "which items are about to decay?", "show me active memories", or provides an event ID and wants to know its attention score.
---

# Attention Logger

This skill provides visibility into the `mirror-log` attention layer, allowing you to inspect how the system is deciding which information remains in "active thought" versus what has been demoted to "cold storage."

## Core Functionality

### 1. Inspecting Active Items
When asked to show active items, use the `AttentionLayer::get_active_items` method from `mirror-log/src/attention/mod.rs`. 
- Focus on presenting the `id`, `source`, `content` (snippet), and `last_accessed` timestamp.
- Highlight "pinned" items as they are immune to decay.

### 2. Monitoring Decay (The "Flagged" List)
Use `AttentionLayer::get_flagged_items` to identify events that are approaching the decay threshold.
- This is crucial for understanding what information the agent is about to "forget."
- Present these items with their `access_count` and `last_accessed` duration to show how close they are to falling out of attention.

### 3. Analyzing Statistics
Use `AttentionLayer::get_stats` to provide a high-level overview of the system health:
- **Total Events**: The size of the entire log.
- **Active Events**: The current working set in the attention layer.
- **Pinned Events**: The "evergreen" knowledge base.
- **Flagged Events**: The at-risk population.
- **Active Percentage**: A key metric for monitoring context density.

### 4. Evaluating Individual Importance
If a specific `event_id` is provided, use `AttentionLayer::calculate_attention_score`.
- Translate the raw score into a human-readable context (e.g., "High priority/recent" vs "Low priority/stale").

## Workflow Instructions

1.  **Identify the Intent**: Determine if the user wants a broad overview (stats), a list of specific items (active/flagged), or an evaluation of a single item.
2.  **Access the Database**: Use the existing `mirror-log` connection logic to interact with the SQLite backend.
3.  **Format the Output**: 
    - For lists, use Markdown tables for readability.
    - For stats, use a structured list or a summary block.
    - Always include human-readable durations (e.g., "2h ago" instead of raw Unix timestamps) by leveraging the `last_accessed_str` methods available in the `AttentionItem` struct.
4.  **Contextualize**: If you notice a high number of flagged items, suggest that a "consolidation" or "re-indexing" might be necessary to prevent loss of context.

## Reference Material

- For the underlying logic and SQL queries, refer to: `mirror-log/src/attention/mod.rs`
- For understanding how decay is calculated, see: `mirror-log/src/decay.rs`
