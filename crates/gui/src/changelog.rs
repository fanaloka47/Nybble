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
        version: "1.6.0",
        items: &[
            "Added a Scratchpad tab for free-form notes that's saved with the rest of your preferences and restored the next time you open the app.",
            "Float mode's decimal display now groups digits with thousands separators, matching the integer bases.",
            "Your Copy Options now apply to native Ctrl+C copies from the value fields, not just the Copy buttons. Can be changed in settings",
            "Fixed: on Windows, the in-app updater could fail to launch the downloaded installer because the command line was over-escaped.",
        ],
    },
    ReleaseNotes {
        version: "1.5.0",
        items: &[
            "Nybble now has real installers. On Windows there's an .msi that adds Start Menu and Add/Remove Programs entries; on Linux there's a .deb for Debian/Ubuntu and an AppImage that runs anywhere. The portable downloads are still there and still work exactly as before.",
            "Your settings and history are shared across every version. Install the Windows or Linux package over a portable copy and everything you had is still there — and uninstalling never deletes it.",
            "The update button now matches how you installed. Portable and AppImage builds update themselves as before; the Windows installer downloads and runs the new installer for you; a version installed through your Linux package manager points you at the download page instead of writing files behind your package manager's back.",
            "Fixed: a failed update left the button stuck on \"Updating…\" until you restarted the app.",
            "Fixed: when there was nothing new to install, the app would restart anyway and show the same update prompt again.",
        ],
    },
    ReleaseNotes {
        version: "1.4.0",
        items: &[
            "Your calculation history now persists across sessions: it's saved when you close the app and restored the next time you open it, keeping the 50 most recent entries. The current value still starts fresh on each launch.",
            "The integer/float toggle is now a compact pill directly under the Calculator tab.",
            "In float mode, the HEX/BIN/OCT fields are greyed out and point to a new Interpret panel that decodes the IEEE-754 bit pattern of the current value.",
        ],
    },
    ReleaseNotes {
        version: "1.3.0",
        items: &[
            "Added a Batch convert tab: paste a whole list of numbers and convert them between bases at once, with a Copy all button.",
            "The batch converter auto-detects the source base of your list (or you can pick one), and shows the detected base and a value/error count.",
            "The HEX/DEC/BIN/OCT value fields now accept full expressions, evaluated in each field's own base — e.g. DEAD & 0xF0 in hex, 1 << 3 in binary, or ans * 2 anywhere.",
            "Value fields now re-format with group separators as soon as you leave the field or press Enter.",
            "Added an About tab in Settings showing the version, license, and third-party attribution.",
            "Redesigned the app icon as a colored abacus.",
        ],
    },
    ReleaseNotes {
        version: "1.2.0",
        items: &[
            "Added a setting to send an expression's result back into the input field in decimal, ready to build the next expression on (Settings → Expressions; off by default).",
            "Added a Clear button to the bit grid header.",
            "History entries now have buttons to copy the result or send it back to the expression.",
            "Decimal values now group digits with thousands separators for readability.",
            "Switching from float to integer mode now keeps the value when the result is a whole number.",
            "Pressing Enter to evaluate no longer makes the expression field flicker.",
        ],
    },
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
