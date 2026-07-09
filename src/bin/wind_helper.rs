#[cfg(all(feature = "cef-renderer", target_os = "macos"))]
fn main() {
    let executable_path = std::env::current_exe().expect("helper executable path");
    let loader = cef::library_loader::LibraryLoader::new(&executable_path, true);

    if !loader.load() {
        eprintln!("failed to load CEF framework for helper process");
        std::process::exit(1);
    }

    let args = cef::args::Args::new();
    let result = cef::execute_process(Some(args.as_main_args()), None, std::ptr::null_mut());

    if result >= 0 {
        std::process::exit(result);
    }
}

#[cfg(not(all(feature = "cef-renderer", target_os = "macos")))]
fn main() {}
