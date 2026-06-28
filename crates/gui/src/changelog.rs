//! Embedded release notes shown in the "What's new" dialog.
//!
//! The notes ship inside the binary so the dialog works offline. After an
//! update, `App` compares the running version against the persisted
//! `last_seen_version` and surfaces the matching entry once.

/// The notes for a single released version.
pub struct ReleaseNotes {
    pub version: &'static str,
    pub items: &'static [&'static str],
}

/// Every release's notes, newest first. Add a new entry at the top — and bump
/// `crates/gui/Cargo.toml`'s `version` to match — each time you cut a release.
pub const ENTRIES: &[ReleaseNotes] = &[
    ReleaseNotes {
        version: "1.1.0",
        items: &[
            "Added a “What's new” dialog that appears after updating.",
            "Click the version number in the header to reopen it anytime.",
        ],
    },
    ReleaseNotes {
        version: "1.0.0",
        items: &["First release."],
    },
];

/// The notes for an exact version, if present.
pub fn notes_for(version: &str) -> Option<&'static ReleaseNotes> {
    ENTRIES.iter().find(|e| e.version == version)
}
