//! Contains traits with platform-specific methods in them.
//!
//! Only the modules corresponding to the platform you're compiling to will be available.

// NOTE (vendored, trimmed): the android/ios/macos/orbital/web public
// extension modules were removed; only Linux and Windows are kept (see NOTICE).
// `docsrs` was dropped from the cfgs since the removed modules can no longer be
// doc-built.
#[cfg(any(x11_platform, wayland_platform))]
pub mod startup_notify;
#[cfg(wayland_platform)]
pub mod wayland;
#[cfg(windows_platform)]
pub mod windows;
#[cfg(x11_platform)]
pub mod x11;

#[cfg(any(
    windows_platform,
    macos_platform,
    android_platform,
    x11_platform,
    wayland_platform,
    docsrs,
))]
pub mod run_on_demand;

#[cfg(any(
    windows_platform,
    macos_platform,
    android_platform,
    x11_platform,
    wayland_platform,
    docsrs,
))]
pub mod pump_events;

#[cfg(any(
    windows_platform,
    macos_platform,
    x11_platform,
    wayland_platform,
    orbital_platform,
    docsrs
))]
pub mod modifier_supplement;

#[cfg(any(windows_platform, macos_platform, x11_platform, wayland_platform, docsrs))]
pub mod scancode;
