fn main() {
    #[cfg(windows)]
    {
        winresource::WindowsResource::new()
            .set_icon("../../assets/icon.ico")
            .compile()
            .unwrap();
    }
}
