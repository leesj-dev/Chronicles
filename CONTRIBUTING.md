# Contributing

The Rust crate in `engine/` owns collection, HTTP, HTML export, and SSH sync. Changes to report conversion should include small fictional fixtures that show accounting and deduplication. Keep actual usage snapshots, conversation text, credentials, and private screenshots outside the repository.

Keep browser code dependency-free, serve assets locally, and use the shared analytics for all chart views. Preserve accessible filtering, empty states, and missing-data semantics. Run the checks in README.md and verify affected UI behavior in a browser before opening a pull request.
