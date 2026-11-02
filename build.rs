fn main() {
    let target = std::env::var("TARGET").unwrap_or_default();
    if !target.contains("windows") {
        return;
    }

    use embed_manifest::manifest::{ActiveCodePage, Setting, SupportedOS::*};
    use embed_manifest::{embed_manifest, new_manifest};

    let manifest = new_manifest("Thrive.Mastodon")
        .supported_os(Windows7..=Windows10)
        .active_code_page(ActiveCodePage::Utf8)
        .dpi_awareness(embed_manifest::manifest::DpiAwareness::PerMonitorV2)
        .long_path_aware(Setting::Enabled);

    if let Err(error) = embed_manifest(manifest) {
        println!("cargo:warning=Failed to embed manifest: {error}");
    }
    println!("cargo:rerun-if-changed=build.rs");
}
