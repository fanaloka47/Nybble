fn main() {
    #[cfg(windows)]
    {
        // Per-Monitor V2 DPI awareness: render crisply at each monitor's native
        // scale. This is only safe because of the patched winit in
        // `vendor/winit` (a backport of upstream commit 488c036a, "Fixes
        // #4041", wired via [patch.crates-io] in the workspace Cargo.toml) —
        // unpatched winit 0.30 mishandles the per-monitor DPI handoff and the
        // window ping-pongs/grows when dragged across a scale boundary. When
        // eframe adopts a winit release containing the fix, the vendored copy
        // and the [patch] entry can be dropped with no change here.
        const MANIFEST: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
  <trustInfo xmlns="urn:schemas-microsoft-com:asm.v3">
    <security>
      <requestedPrivileges>
        <requestedExecutionLevel level="asInvoker" uiAccess="false"/>
      </requestedPrivileges>
    </security>
  </trustInfo>
  <application xmlns="urn:schemas-microsoft-com:asm.v3">
    <windowsSettings>
      <dpiAware xmlns="http://schemas.microsoft.com/SMI/2005/WindowsSettings">true/PM</dpiAware>
      <dpiAwareness xmlns="http://schemas.microsoft.com/SMI/2016/WindowsSettings">PerMonitorV2, PerMonitor</dpiAwareness>
    </windowsSettings>
  </application>
</assembly>
"#;
        winresource::WindowsResource::new()
            .set_icon("../../assets/icon.ico")
            .set_manifest(MANIFEST)
            .compile()
            .unwrap();
    }
}
