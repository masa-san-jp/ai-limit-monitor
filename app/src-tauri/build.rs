fn main() {
    // アプリ独自コマンドをパーミッション管理下に置く(allow-<command> が生成される)。
    // Grok の WebView(リモートページ)には grok_report だけを許可する。
    tauri_build::try_build(
        tauri_build::Attributes::new().app_manifest(
            tauri_build::AppManifest::new().commands(&[
                "get_snapshot",
                "refresh_now",
                "open_grok_login",
                "quit_app",
                "grok_report",
            ]),
        ),
    )
    .expect("tauri build failed");
}
