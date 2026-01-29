use crate::error::Result;
use crate::state::{self, Entry};
use crate::tmux;
use std::collections::HashSet;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq)]
pub struct StaleEntry {
    pub id: Uuid,
    pub branch: String,
    pub reasons: Vec<String>,
}

pub fn execute(dry_run: bool) -> Result<()> {
    let state = state::load()?;

    let stale_entries = find_stale_entries(
        &state.entries,
        |e| e.path.exists(),
        |e| tmux::window_exists(&e.tmux_session, &e.tmux_window).unwrap_or(false),
    );

    if stale_entries.is_empty() {
        println!("No stale entries found.");
        return Ok(());
    }

    println!("Found {} stale entries:", stale_entries.len());
    for entry in &stale_entries {
        println!("  {} ({})", entry.branch, entry.reasons.join(", "));
    }

    if dry_run {
        println!("\nDry run - no changes made.");
    } else {
        for entry in stale_entries {
            state::remove_entry(entry.id)?;
        }
        println!("\nRemoved stale entries from state.");
    }

    Ok(())
}

/// Finds stale entries based on provided check functions.
/// An entry is stale if:
/// - path_exists returns false
/// - window_exists returns false
/// - it's a duplicate branch (second or later occurrence)
pub fn find_stale_entries<F, G>(entries: &[Entry], path_exists: F, window_exists: G) -> Vec<StaleEntry>
where
    F: Fn(&Entry) -> bool,
    G: Fn(&Entry) -> bool,
{
    let mut stale_entries: Vec<StaleEntry> = Vec::new();
    let mut seen_branches: HashSet<&str> = HashSet::new();

    for entry in entries {
        let mut reasons: Vec<String> = Vec::new();

        if !path_exists(entry) {
            reasons.push("worktree missing".to_string());
        }
        if !window_exists(entry) {
            reasons.push("window missing".to_string());
        }
        if seen_branches.contains(entry.branch.as_str()) {
            reasons.push("duplicate branch".to_string());
        }

        if !reasons.is_empty() {
            stale_entries.push(StaleEntry {
                id: entry.id,
                branch: entry.branch.clone(),
                reasons,
            });
        }

        seen_branches.insert(&entry.branch);
    }

    stale_entries
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{Command, Entry};
    use chrono::Utc;
    use std::path::PathBuf;

    fn make_entry(id: Uuid, branch: &str) -> Entry {
        Entry {
            id,
            project: "test".to_string(),
            branch: branch.to_string(),
            path: PathBuf::from("/tmp/test"),
            tmux_session: "0".to_string(),
            tmux_window: branch.to_string(),
            command: Command::Raw {
                cmd: "echo test".to_string(),
            },
            exit_kill: None,
            exit_code: None,
            created_at: Utc::now(),
        }
    }

    #[test]
    fn test_no_stale_entries_when_all_valid() {
        let id1 = Uuid::new_v4();
        let id2 = Uuid::new_v4();
        let entries = vec![
            make_entry(id1, "feature-a"),
            make_entry(id2, "feature-b"),
        ];

        let stale = find_stale_entries(&entries, |_| true, |_| true);
        assert!(stale.is_empty());
    }

    #[test]
    fn test_stale_when_path_missing() {
        let id = Uuid::new_v4();
        let entries = vec![make_entry(id, "feature-a")];

        let stale = find_stale_entries(&entries, |_| false, |_| true);

        assert_eq!(stale.len(), 1);
        assert_eq!(stale[0].id, id);
        assert!(stale[0].reasons.contains(&"worktree missing".to_string()));
    }

    #[test]
    fn test_stale_when_window_missing() {
        let id = Uuid::new_v4();
        let entries = vec![make_entry(id, "feature-a")];

        let stale = find_stale_entries(&entries, |_| true, |_| false);

        assert_eq!(stale.len(), 1);
        assert_eq!(stale[0].id, id);
        assert!(stale[0].reasons.contains(&"window missing".to_string()));
    }

    #[test]
    fn test_stale_when_both_missing() {
        let id = Uuid::new_v4();
        let entries = vec![make_entry(id, "feature-a")];

        let stale = find_stale_entries(&entries, |_| false, |_| false);

        assert_eq!(stale.len(), 1);
        assert_eq!(stale[0].reasons.len(), 2);
        assert!(stale[0].reasons.contains(&"worktree missing".to_string()));
        assert!(stale[0].reasons.contains(&"window missing".to_string()));
    }

    #[test]
    fn test_duplicate_branch_marks_second_as_stale() {
        let id1 = Uuid::new_v4();
        let id2 = Uuid::new_v4();
        let entries = vec![
            make_entry(id1, "feature-a"),
            make_entry(id2, "feature-a"), // duplicate
        ];

        let stale = find_stale_entries(&entries, |_| true, |_| true);

        assert_eq!(stale.len(), 1);
        assert_eq!(stale[0].id, id2); // second one is stale
        assert!(stale[0].reasons.contains(&"duplicate branch".to_string()));
    }

    #[test]
    fn test_first_valid_duplicate_with_missing_path_still_kept() {
        // First entry has valid path/window, second is duplicate
        // Only the second should be marked stale (as duplicate)
        let id1 = Uuid::new_v4();
        let id2 = Uuid::new_v4();
        let entries = vec![
            make_entry(id1, "feature-a"),
            make_entry(id2, "feature-a"),
        ];

        let stale = find_stale_entries(&entries, |_| true, |_| true);

        assert_eq!(stale.len(), 1);
        assert_eq!(stale[0].id, id2);
    }

    #[test]
    fn test_orphaned_first_entry_and_valid_second_both_issues() {
        // First entry: window missing (orphaned from failed creation)
        // Second entry: duplicate branch but otherwise valid
        let id1 = Uuid::new_v4();
        let id2 = Uuid::new_v4();
        let entries = vec![
            make_entry(id1, "feature-a"),
            make_entry(id2, "feature-a"),
        ];

        // First entry has no window, second has window
        let stale = find_stale_entries(
            &entries,
            |_| true,
            |e| e.id == id2, // only second has window
        );

        assert_eq!(stale.len(), 2);
        // First is stale due to window missing
        assert_eq!(stale[0].id, id1);
        assert!(stale[0].reasons.contains(&"window missing".to_string()));
        // Second is stale due to duplicate
        assert_eq!(stale[1].id, id2);
        assert!(stale[1].reasons.contains(&"duplicate branch".to_string()));
    }

    #[test]
    fn test_valid_entry_not_marked_stale() {
        let id1 = Uuid::new_v4();
        let id2 = Uuid::new_v4();
        let id3 = Uuid::new_v4();
        let entries = vec![
            make_entry(id1, "feature-a"),
            make_entry(id2, "feature-b"), // this one is valid
            make_entry(id3, "feature-c"),
        ];

        // Only id2 has valid path and window
        let stale = find_stale_entries(
            &entries,
            |e| e.id == id2,
            |e| e.id == id2,
        );

        assert_eq!(stale.len(), 2);
        assert!(stale.iter().all(|s| s.id != id2)); // id2 should NOT be in stale list
    }

    #[test]
    fn test_mixed_scenario() {
        let id1 = Uuid::new_v4();
        let id2 = Uuid::new_v4();
        let id3 = Uuid::new_v4();
        let id4 = Uuid::new_v4();
        let entries = vec![
            make_entry(id1, "feature-a"), // valid
            make_entry(id2, "feature-b"), // path missing
            make_entry(id3, "feature-a"), // duplicate of id1
            make_entry(id4, "feature-c"), // valid
        ];

        let stale = find_stale_entries(
            &entries,
            |e| e.id != id2, // id2 has missing path
            |_| true,
        );

        assert_eq!(stale.len(), 2);

        let stale_ids: Vec<Uuid> = stale.iter().map(|s| s.id).collect();
        assert!(stale_ids.contains(&id2)); // path missing
        assert!(stale_ids.contains(&id3)); // duplicate
        assert!(!stale_ids.contains(&id1)); // valid
        assert!(!stale_ids.contains(&id4)); // valid
    }
}
