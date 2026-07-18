# CEF Renderer

Wind uses a small renderer boundary in `src/renderer` so the egui shell does not depend on CEF APIs directly.

The default build uses the CEF/Chromium renderer:

```sh
cargo run
```

On macOS, `cargo run` automatically launches a persistent development app bundle.
The first run creates the CEF bundle; subsequent runs only copy the rebuilt Wind
and helper executables into it, rather than recopying the CEF framework. The
bundle is recreated when its Cargo inputs, CEF source path, or assets change.

To deliberately rebuild the bundle, remove `target/debug/bundle/wind.app` and
run `cargo run` again.

CEF subprocesses require a real Apple development signature on recent macOS
releases. Wind automatically uses the first `Apple Development` identity in the
login keychain. Confirm Xcode created one with:

```sh
security find-identity -v -p codesigning
```

To select a different identity explicitly, set its exact name:

```sh
export WIND_CODESIGN_IDENTITY='Apple Development: Your Name (TEAMID)'
```

Without a valid identity, Wind falls back to an ad-hoc signature and Chromium
may log a process-signature validation error.

The placeholder renderer is only available for quick shell work with:

```sh
cargo run --no-default-features
```

Native CEF builds require CMake and Ninja:

```sh
brew install cmake ninja
```

CEF ships large native binaries. The `cef` crate can download them during build, but local iteration is faster when the shared CEF directory is exported once and reused:

From a `cef-rs` checkout, run:

```sh
cargo run -p export-cef-dir -- --force "$HOME/.local/share/cef"
export CEF_PATH="$HOME/.local/share/cef"
```

On macOS also expose the framework libraries:

```sh
export DYLD_FALLBACK_LIBRARY_PATH="$DYLD_FALLBACK_LIBRARY_PATH:$CEF_PATH:$CEF_PATH/Chromium Embedded Framework.framework/Libraries"
```

The renderer owns a native Chromium child view for each live HTTP tab and presents only the Active Tab. Tab selection, navigation, and reload are expressed as a `PageTarget` containing the Space and Tab identities, URL, render revision, and physical bounds. Title, URL, and favicon callbacks flow back through the renderer boundary; downloads, popup routing, and additional platform polish can remain behind the same boundary.

Each Wind Space owns a persistent CEF request context rooted below Wind's application-data directory. Every tab created in a Space uses that context, so cookies and HTML5 storage survive restarts within the Space while remaining isolated from other Spaces. Inactive Space views remain alive but hidden until their tabs are closed, moved, or the Space is deleted.
