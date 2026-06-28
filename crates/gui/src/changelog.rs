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
            "Added a power operator (**) and named functions (sqrt, log2, clog2, gcd, …) to integer expressions.",
            "Added scientific functions and constants (pi, e, tau) to float mode.",
            "Added a Settings panel: reorder or hide sections, toggle per-field options, and tune copy behaviour, with a built-in expression reference.",
            "Added a per-field button to send any value straight into the expression.",
            "Copying a value now strips underscores and includes the base prefix.",
            "Long expressions now wrap instead of scrolling off-screen.",
            "Window sizing is now DPI-aware for multi-monitor setups.",
            "Added a “What's new” dialog that appears after updating — click the version number in the header to reopen it anytime.",
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
