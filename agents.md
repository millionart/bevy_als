# bevy_als

This crate owns the ALS runtime. The public entry points are `bevy_als::AlsPlugin` and `spawn_als_character`. Character code lives in `src/als`, camera code in `src/als_camera`, and dependency extensions in the corresponding `src/*_ext` modules. The separate `bevy_als_playground` project owns the demo scene, input adapter, and scene automation.

- Preserve the public plugin, installed assets, RON reflection identities, and host schedules. Prefer public Bevy and dependency APIs.
- The external Bevy checkout is read-only. The user owns its files, Git data, and permissions. Put project-specific changes in this repository.
- Reproduce bugs with a focused failing case before fixing them. Choose regression checks for the affected systems, assets, and callers. Run broad Cargo commands with `--jobs 2` from the project root.
- Keep JSON regression fixtures in `src/tests/fixtures/` with at most one module directory. Name them for their behavior and data. Preserve test inputs, thresholds, and assertions. Keep original captures, logs, and local paths out of the fixtures.
- Run the core suite with `cargo test --locked --jobs 2 -- --test-threads=1`. See the companion Playground README for app and scene checks.
- Use English in repository text, preserve relative asset paths and source licenses, and keep locally generated build files and settings out of commits.
