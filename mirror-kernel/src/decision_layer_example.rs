// Git Decision Layer Example
// Comprehensive demonstration of the Git-based decision layer for Mirror Kernel

use crate::{DecisionBlob, DecisionTree, GitDecisionLayer, GitError, MirrorTag, Reflection};
use chrono::{DateTime, Utc};
use std::path::PathBuf;

fn main() -> Result<(), GitError> {
    println!("=== Git Decision Layer Example ===\n");

    // Step 1: Create a new Git decision layer
    let repo_path = PathBuf::from("/tmp/mirror_decision_repo");
    println!("1. Creating Git decision layer at {}", repo_path.display());

    let decision_layer = GitDecisionLayer::new(&repo_path)?;
    println!("   ✓ Decision layer created successfully\n");

    // Step 2: Create sample reflections (proposals)
    println!("2. Creating sample reflections");

    let reflection1 = Reflection {
        new_content: "Empathic response: I understand you're stressed and need support.",
        new_tags: vec![MirrorTag::Reflect, MirrorTag::EmpathicHigh],
    };
    println!("   ✓ Created: {}", reflection1.new_content);

    let reflection2 = Reflection {
        new_content: "Critical analysis: Let's examine the root cause of this stress.",
        new_tags: vec![MirrorTag::Challenge, MirrorTag::Reflect],
    };
    println!("   ✓ Created: {}", reflection2.new_content);

    let reflection3 = Reflection {
        new_content: "Compressed summary: User experiencing stress levels requiring empathy.",
        new_tags: vec![MirrorTag::Compress, MirrorTag::Reflect],
    };
    println!("   ✓ Created: {}", reflection3.new_content);

    println!();

    // Step 3: Commit decisions to Git
    println!("3. Committing decisions to Git repository");

    let context_tags = vec![MirrorTag::Reflect, MirrorTag::EmpathicLow];

    // Commit decision 1
    let commit_hash1 = decision_layer.commit_decision(
        &reflection1,
        &context_tags,
        "empathic_mirror",
        "User reported stress, appropriate empathic response",
        &[101, 102, 103],
    )?;
    println!("   ✓ Decision 1 committed: {}", commit_hash1);

    // Commit decision 2
    let commit_hash2 = decision_layer.commit_decision(
        &reflection2,
        &[MirrorTag::Challenge],
        "challenge_mirror",
        "Need to analyze root cause of stress",
        &[101, 102],
    )?;
    println!("   ✓ Decision 2 committed: {}", commit_hash2);

    // Commit decision 3
    let commit_hash3 = decision_layer.commit_decision(
        &reflection3,
        &[MirrorTag::Compress],
        "compress_mirror",
        "Need concise summary of user state",
        &[101],
    )?;
    println!("   ✓ Decision 3 committed: {}", commit_hash3);

    println!();

    // Step 4: Retrieve all decisions
    println!("4. Retrieving all decisions from Git history");

    let all_decisions = decision_layer.get_all_decisions()?;
    println!("   ✓ Found {} decisions in history:", all_decisions.len());

    for (i, decision) in all_decisions.iter().enumerate() {
        println!(
            "      {}. {} - {} ({})",
            i + 1,
            decision.kernel_name,
            decision.reason,
            decision.timestamp.format("%Y-%m-%d %H:%M:%S")
        );
    }

    println!();

    // Step 5: Query decisions by kernel
    println!("5. Querying decisions by kernel");

    let empathic_decisions = decision_layer.get_decisions_by_kernel("empathic_mirror")?;
    println!("   ✓ Empathic decisions: {}", empathic_decisions.len());

    let challenge_decisions = decision_layer.get_decisions_by_kernel("challenge_mirror")?;
    println!("   ✓ Challenge decisions: {}", challenge_decisions.len());

    let compress_decisions = decision_layer.get_decisions_by_kernel("compress_mirror")?;
    println!("   ✓ Compress decisions: {}", compress_decisions.len());

    println!();

    // Step 6: Query decisions by tag
    println!("6. Querying decisions by tag");

    let empathic_high_decisions = decision_layer.get_decisions_by_tag(&MirrorTag::EmpathicHigh)?;
    println!(
        "   ✓ Empathic High decisions: {}",
        empathic_high_decisions.len()
    );

    let challenge_decisions = decision_layer.get_decisions_by_tag(&MirrorTag::Challenge)?;
    println!("   ✓ Challenge decisions: {}", challenge_decisions.len());

    println!();

    // Step 7: Get decision history (chronological)
    println!("7. Getting chronological decision history");

    let history = decision_layer.get_decision_history()?;
    println!("   ✓ History sorted by timestamp:");

    for (i, decision) in history.iter().enumerate() {
        println!(
            "      {}. {} - {}",
            i + 1,
            decision.timestamp.format("%Y-%m-%d %H:%M:%S"),
            decision.commit_hash
        );
    }

    println!();

    // Step 8: Build and query decision tree
    println!("8. Building decision tree (DAG structure)");

    let decision_tree = decision_layer.get_decision_tree()?;
    println!(
        "   ✓ Decision tree created with {} total decisions:",
        decision_tree.size()
    );

    println!("   Decision tree statistics:");
    println!("      - Total decisions: {}", decision_tree.size());
    println!("      - Unique commits: {}", decision_tree.by_commit.len());
    println!("      - Kernels used: {}", decision_tree.by_kernel.len());

    println!();

    // Step 9: Create exploratory branch
    println!("9. Creating exploratory branch for experimentation");

    decision_layer.create_branch("experimental_decisions")?;
    println!("   ✓ Created branch 'experimental_decisions'");

    // Commit a decision to the experimental branch
    let experimental_reflection = Reflection {
        new_content: "Experimental approach: Let's try a different empathic strategy.",
        new_tags: vec![MirrorTag::Reflect, MirrorTag::EmpathicLow],
    };

    let experimental_commit = decision_layer.commit_decision(
        &experimental_reflection,
        &[MirrorTag::Reflect],
        "empathic_mirror",
        "Testing alternative empathic approach",
        &[104],
    )?;
    println!(
        "   ✓ Experimental decision committed: {}",
        experimental_commit
    );

    println!();

    // Step 10: Merge experimental branch
    println!("10. Merging experimental branch into main");

    decision_layer.merge_branch("experimental_decisions")?;
    println!("   ✓ Experimental decisions merged successfully");

    // Verify all decisions are now accessible
    let final_decisions = decision_layer.get_all_decisions()?;
    println!(
        "   ✓ Total decisions after merge: {}",
        final_decisions.len()
    );

    println!();

    // Step 11: Display commit log
    println!("11. Displaying Git commit log");

    let output = std::process::Command::new("git")
        .args(["-C", repo_path.to_str().unwrap(), "log", "--oneline"])
        .output()?;

    if output.status.success() {
        let log = String::from_utf8(output.stdout)?;
        println!("   Git log:");
        for line in log.lines().take(10) {
            println!("      {}", line);
        }
    }

    println!();
    println!("=== Example completed successfully ===");

    Ok(())
}

// Additional helper functions for demonstration

/// Demonstrate decision layer with custom event IDs
fn demonstrate_custom_event_ids() -> Result<(), GitError> {
    let repo_path = PathBuf::from("/tmp/mirror_decision_custom");
    let decision_layer = GitDecisionLayer::new(&repo_path)?;

    let reflection = Reflection {
        new_content: "Custom event handling: Processing event ID 999",
        new_tags: vec![MirrorTag::Reflect],
    };

    let commit_hash = decision_layer.commit_decision(
        &reflection,
        &[MirrorTag::Reflect],
        "custom_kernel",
        "Handling custom event with ID 999",
        &[999],
    )?;

    println!("Custom event decision committed: {}", commit_hash);
    Ok(())
}

/// Demonstrate error handling
fn demonstrate_error_handling() {
    println!("Demonstrating error handling scenarios:");

    // Error case 1: Invalid path
    let invalid_path = PathBuf::from("/invalid/path");
    match GitDecisionLayer::new(&invalid_path) {
        Ok(_) => println!("   ✓ Valid path succeeded"),
        Err(e) => println!("   ✗ Invalid path failed: {}", e),
    }

    // Error case 2: Invalid git command
    // (This would require a more complex setup to demonstrate)

    println!("   Note: Error handling is demonstrated in the main function");
}

/// Demonstrate decision blob structure
fn demonstrate_decision_blob_structure() {
    println!("Decision blob structure:");
    println!("  - selected_reflection: Reflection (new_content, new_tags)");
    println!("  - context_tags: Vec<MirrorTag>");
    println!("  - kernel_name: String");
    println!("  - reason: String");
    println!("  - event_ids: Vec<i64>");
    println!("  - timestamp: DateTime<Utc>");
    println!("  - commit_hash: String");
    println!("  - All fields are serializable/deserializable");
}
