#[cfg(all(feature = "cef-renderer", target_os = "macos"))]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use cef::build_util::mac::{BundleInfo, bundle};
    use semver::Version;
    use std::path::{Path, PathBuf};

    let bundle_info = BundleInfo {
        name: "wind".to_string(),
        identifier: "dev.wind.browser".to_string(),
        display_name: "Wind".to_string(),
        development_region: "English".to_string(),
        version: Version::parse(env!("CARGO_PKG_VERSION"))?,
    };

    let profile = if cfg!(debug_assertions) {
        "debug"
    } else {
        "release"
    };
    let target_path = Path::new("target").join(profile);
    let output_path = target_path.join("bundle");
    let bundle_path = bundle(
        &output_path,
        &target_path,
        "wind",
        "wind_helper",
        Some(PathBuf::from("assets")),
        bundle_info,
    )?;
    sign_bundle(&bundle_path)?;

    println!("Run the bundled app from {}", bundle_path.display());
    Ok(())
}

#[cfg(all(feature = "cef-renderer", target_os = "macos"))]
fn sign_bundle(bundle_path: &std::path::Path) -> Result<(), Box<dyn std::error::Error>> {
    let status = std::process::Command::new("codesign")
        .args(["--force", "--deep", "--sign", "-"])
        .arg(bundle_path)
        .status()?;

    if status.success() {
        Ok(())
    } else {
        Err(format!("codesign failed for {}", bundle_path.display()).into())
    }
}

#[cfg(not(all(feature = "cef-renderer", target_os = "macos")))]
fn main() {
    eprintln!("bundle_wind is only available on macOS with the cef-renderer feature enabled");
    std::process::exit(1);
}
