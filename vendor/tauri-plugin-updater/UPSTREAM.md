# Vendored Tauri updater

This directory contains the Rust crate from
[`tauri-apps/plugins-workspace` at `3d836244bcb97cb1620405e8d1887da285a793d4`](https://github.com/tauri-apps/plugins-workspace/tree/3d836244bcb97cb1620405e8d1887da285a793d4/plugins/updater),
version `3.0.0-alpha.3`, for the isolated CEF prototype.

The upstream Rust sources, build script, embedded JavaScript, permissions, documentation,
and Apache-2.0/MIT license files are preserved byte for byte. `UPSTREAM.json` records
all original Git blob hashes. Files excluded by the upstream crate manifest are omitted;
its nested example applications and migration fixtures are not part of this vendored crate.
Upstream unit tests inside the Rust sources remain included.

`Cargo.toml` has these changes:

- Expand workspace package metadata and dependencies so this crate does not inherit
  Liberte's edition, authors, license, or unrelated dependency versions.
- Pin `tauri =3.0.0-alpha.5` and build dependency `tauri-plugin =3.0.0-alpha.4`,
  matching [Tauri tag `tauri-v3.0.0-alpha.5`](https://github.com/tauri-apps/tauri/tree/1991873f5e2baa68773f21465f61baa93077498f).
  Tauri's `default-features = false` setting is retained. The other dependency
  requirements and feature flags remain the upstream values.
- Apply the exact `zip` dependency change from official commit
  [`ba98bfadc4c868c9a7e1a226c83af528e1f11b89`](https://github.com/tauri-apps/plugins-workspace/commit/ba98bfadc4c868c9a7e1a226c83af528e1f11b89):
  `version = "4"` becomes `version = "9"`, with default features still disabled
  and the dependency still optional. That upstream commit contains no Rust source changes.

No lockfile is copied or edited here. The application workspace lockfile is generated
and validated in CI. The upstream signature verification, HTTPS checks, installer path
handling, and permission definitions are unchanged by this vendoring operation.
