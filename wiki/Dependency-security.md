# Dependency security

Reviewed on 2026-10-22. The first integration build reported 18 npm findings
(2 critical, 12 high); that count alone does not identify the affected dependency
paths. CI preserves the full `npm audit --json` report so each remaining finding
can be checked against the generated lockfile.

## Targeted updates

| Package | Previous manifest / repository lock | Required floor | Reason |
| --- | --- | --- | --- |
| Vite | `^6` / 6.4.1 | 6.4.5 | File access fixes, including the 6.4.4 query-handling regression fixed on October 21. |
| Vitest | `^3.2.0` / newly introduced | 4.1.12 | Fixes the affected `@vitest/mocker` dependency without overriding its internal API across major versions. |
| PostCSS | `^8` / 8.5.6 | 8.5.23 | Fixes source-map file disclosure, including the incomplete earlier correction. |
| Rollup | transitive / 4.57.1 | 4.59.0 | Fixes output path traversal and arbitrary file writes. |
| Tailwind CSS | `^3` / 3.4.19 | 4.3.3 | Removes the unpatched `braces` chain through `chokidar`, `fast-glob`, and `micromatch`, plus the old selector parser. |
| Babel core | transitive / 7.29.0 | 7.29.6 | Fixes source-map file reads when compiling untrusted source. |
| Browserslist | transitive / 4.28.6 | 4.28.7 | Fixes query-cache memory exhaustion and unsafe statistics parsing. |
| baseline-browser-mapping | transitive / below 2.11.0 | 2.11.0 | Invalid input throws instead of terminating the process. |
| Picomatch | transitive / 2.3.1 and 4.0.3 | 2.3.2 and 4.0.4 | Fixes crafted-pattern denial of service while keeping each consumer's major version. |

The manifest keeps Vite 6 and PostCSS 8. Its overrides reuse the direct Vite and
PostCSS specifications and constrain Rollup to the patched 4.x line, preventing
an older nested copy from remaining in the lockfile. LiveKit stays at 2.23.0.
No `npm audit fix --force` or guessed lockfile integrity values are used.

The third integration run (`39697891902`, commit `13188d2`) confirmed 10 remaining
findings: 7 high, 2 moderate, 1 low, and no critical findings. The package paths
above come from its saved `npm audit --json` report. `braces` has no patched
release, so a version override cannot fix it. Tailwind 4 removes that dependency
chain; its official PostCSS plugin also handles imports and prefixing, so the
old Autoprefixer plugin is removed. The existing color/font configuration is
loaded explicitly using `@config`, and renamed utility classes retain their
previous appearance and keyboard-focus behavior.

Tailwind 4 requires a modern webview (Chrome 111+, Safari 16.4+, Firefox 128+).
Desktop platform requirements and installers must match that minimum. Validate
the built UI on its supported webviews before a public release.

The override specifications are written explicitly and match their direct
dependency specifications exactly. The integration runner's npm 10.9.9 failed to
resolve the `$postcss` reference while bootstrapping the older lockfile, so the
manifest does not rely on reference-style overrides.

These are build/test dependencies; exploit conditions vary. For example, several
Vite/Vitest issues require an exposed development or test server. Updating them
still protects developer machines and CI. The production desktop application
does not need to run these servers.

## Primary references

- [Vite 6.4.4 regression, fixed in 6.4.5](https://github.com/vitejs/vite/security/advisories/GHSA-8g5g-mxqh-grgj)
- [Vite file allowlist fix](https://github.com/vitejs/vite/security/advisories/GHSA-rq7h-c2jc-7f22)
- [Vitest mock registration injection, fixed in 4.1.12](https://github.com/vitest-dev/vitest/security/advisories/GHSA-qxg6-89mg-rqv8)
- [Vitest Windows traversal fix](https://github.com/vitest-dev/vitest/security/advisories/GHSA-xp26-3454-mjgp)
- [Vitest 4 migration requirements: Node 20+ and Vite 6+](https://v4.vitest.dev/guide/migration.html)
- [PostCSS source-map correction, fixed in 8.5.23](https://github.com/advisories/GHSA-fxqj-rqcc-2cmp)
- [Rollup path traversal, fixed in 4.59.0](https://github.com/advisories/GHSA-mw96-cpmx-2vgc)
- [Unpatched braces recursion](https://github.com/advisories/GHSA-vfj7-8cjw-p6xm)
- [Tailwind 4 migration and webview requirements](https://tailwindcss.com/docs/upgrade-guide)
- [Babel source-map file read](https://github.com/advisories/GHSA-4x5r-pxfx-6jf8)
- [Browserslist query cache](https://github.com/advisories/GHSA-c83g-rgw3-j3cx)
- [Browserslist statistics parsing](https://github.com/advisories/GHSA-73wf-gq98-2v4g)
- [Baseline browser mapping process exit](https://github.com/advisories/GHSA-w5vr-8v7q-w6rv)
- [Picomatch pattern denial of service](https://github.com/advisories/GHSA-c2c7-rcm5-vvqj)
- [Selector parser quadratic complexity](https://github.com/advisories/GHSA-rj75-hqrm-r3gf)

## Verification

Regenerate and review the candidate `frontend/package-lock.json` through the CI
bootstrap step, then commit that exact lockfile. Verify the installed tree with
`npm ci`, `npm audit --json`, `npm test`, `npm run typecheck`, `npm run build`, and
the browser media tests. A manifest change alone is not evidence that all audit
findings are resolved. The final audit and test results must come from the same
commit and lockfile used for release.

## Native Rust dependencies

Reviewed on 2026-10-26. The RustSec job `119562588188` in integration run
[`39698753266`](https://github.com/Kayfeer/liberte/actions/runs/39698753266)
reported 12 known vulnerabilities against the generated Cargo lockfile for
commit `e364ddc`. Its advisory database revision was
`4b587f00996ebe5c05a73eaef105ef241e876414` (2026-10-21). The complete JSON report
is retained as the `cargo-audit` artifact.

The affected versions and dependency paths identify two upstream migrations:

| Affected dependency | Path in the audited lockfile | Patched version floor |
| --- | --- | --- |
| `h2 0.3.27` | Hickory 0.24; also the optional libp2p UPnP/IGD/Hyper chain | 0.4.16 |
| `hickory-proto 0.24.4` | Direct DoH resolver and libp2p mDNS/DNS | 0.26.1 |
| `ring 0.16.20` | libp2p QUIC/TLS through `rcgen 0.11.3` | 0.17.12 |
| `rustls 0.21.12` | Hickory 0.24 resolver/protocol | 0.23.46 |
| `rustls-webpki 0.101.7` | Hickory's Rustls and `libp2p-tls 0.5.0` | 0.103.16 |
| `zip 4.6.1` | `tauri-plugin-updater 2.14.0` | 9.0.0 |

These are lockfile paths; optional or target-specific packages are not
necessarily compiled in every build. Rustls 0.23.46 and WebPKI 0.103.16 were
already present through newer dependency paths. Updating only those copies
would leave the old vulnerable copies in place.

The workspace now selects **libp2p 0.57.1** and **Hickory resolver 0.26.3**.
The libp2p release uses Hickory 0.26, RCGen 0.13, ring 0.17, Rustls 0.23 and
WebPKI 0.103, and replaces the old LRU dependency with Hashlink. The direct DoH
resolver is migrated to Hickory's current API with `https-ring`, `tokio` and
`webpki-roots`; system DNS fallback remains disabled. Network API adaptations
are included with the dependency change.

The updater is pinned to the official `tauri-apps/plugins-workspace` commit
**`ba98bfadc4c868c9a7e1a226c83af528e1f11b89`**, merged on 2026-10-16 as PR #3698.
That upstream change upgrades its Windows ZIP dependency from major 4 to major
9; its own lockfile resolves ZIP 9.0.3. The stable updater 2.14.0 predates this
fix. The pin preserves automatic updates and the plugin's existing signature
verification. Replace it with a published plugin release containing the same
fix once that release is verified. The vulnerable extraction API is used only
on Windows by this plugin; the narrower application exposure does not justify
keeping a known vulnerable package in the lockfile.

No advisory is ignored or suppressed. The audit runs with an empty ignore list,
and its gate continues to reject known vulnerabilities and incomplete reports.
The original report also contains 9 maintenance warnings and 3 soundness
warnings, retained separately in its JSON. These include Bincode 1.3.3,
GLib 0.18.5 and LRU 0.12.5. They must remain visible for review; a clean known-
vulnerability count alone is not evidence that every maintenance or soundness
concern has been resolved. There was no blocking finding specific to the
storage, Argon2 or backup AEAD dependencies in that report.

The RustSec job `121547515357` in run
[`40315605780`](https://github.com/Kayfeer/liberte/actions/runs/40315605780)
confirmed that the 12 findings above were removed. Its newer advisory database
(`115e418ee9a2ff542fee41db641128fa84483383`, 2026-10-26) reported one newly
published vulnerability: **RUSTSEC-2026-0387**, affecting `crypto-bigint 0.5.5`
and fixed in 0.7.6. Some operations intended to run in constant time could leak
secret-dependent information through timing; exploitability depends on the
algorithm, compiler, CPU and observable side channel.

The only path to that package was `liberte-media` -> `webrtc 0.11` ->
`webrtc-dtls 0.10` -> `p256`/`p384` -> `elliptic-curve 0.13.8` ->
`crypto-bigint 0.5.5`. The native `webrtc` dependency was unused: a complete
search of workspace Rust source and manifests found no imports, calls, aliases
or re-exports of that crate. The `webrtc_peer` module maintains application
state without using it. Actual peer transport is implemented with browser
`RTCPeerConnection` in `frontend/src/lib/mediaSession.ts`; SFU transport uses
`livekit-client` in `frontend/src/lib/sfuSession.ts`.

The unused `webrtc = "0.11"` manifest entry has therefore been removed, eliminating
its unused cryptographic dependency chain. This leaves the browser calls,
LiveKit transport, native CPAL audio and media encryption code intact. No
cryptographic patch, forced incompatible version or advisory exception is
introduced. The new report retains 6 maintenance warnings and the GLib
soundness warning separately; these remain visible for review.

**Validation pending:** a new CI run must regenerate the Cargo lockfile after
removing the unused dependency, run RustSec against that exact lockfile, and
pass Rust tests, peer-network integration tests, browser media tests and native
builds on Windows, macOS and Linux. No clean audit after this removal is claimed
here. The lockfile must be produced by Cargo rather than edited manually.

### Rust primary references

- [libp2p 0.57.1 release](https://github.com/libp2p/rust-libp2p/releases/tag/v0.57.1)
- [Released libp2p dependency versions](https://github.com/libp2p/rust-libp2p/blob/v0.57.1/Cargo.toml)
- [Released libp2p TLS dependency versions](https://github.com/libp2p/rust-libp2p/blob/v0.57.1/transports/tls/Cargo.toml)
- [Official updater ZIP 9 change](https://github.com/tauri-apps/plugins-workspace/commit/ba98bfadc4c868c9a7e1a226c83af528e1f11b89)
- [h2 unbounded DATA frames: RUSTSEC-2026-0258](https://rustsec.org/advisories/RUSTSEC-2026-0258.html)
- [Hickory encoding CPU exhaustion: RUSTSEC-2026-0119](https://rustsec.org/advisories/RUSTSEC-2026-0119.html)
- [ring AES/QUIC panic: RUSTSEC-2025-0009](https://rustsec.org/advisories/RUSTSEC-2025-0009.html)
- [Rustls session-secret Debug output: RUSTSEC-2026-0346](https://rustsec.org/advisories/RUSTSEC-2026-0346.html)
- [Rustls key-exchange downgrade: RUSTSEC-2026-0349](https://rustsec.org/advisories/RUSTSEC-2026-0349.html)
- [WebPKI distrusted intermediates: RUSTSEC-2026-0382](https://rustsec.org/advisories/RUSTSEC-2026-0382.html)
- [WebPKI wildcard constraints: RUSTSEC-2026-0099](https://rustsec.org/advisories/RUSTSEC-2026-0099.html)
- [WebPKI URI constraints: RUSTSEC-2026-0098](https://rustsec.org/advisories/RUSTSEC-2026-0098.html)
- [WebPKI CRL parsing panic: RUSTSEC-2026-0104](https://rustsec.org/advisories/RUSTSEC-2026-0104.html)
- [ZIP privileged permissions: RUSTSEC-2026-0367](https://rustsec.org/advisories/RUSTSEC-2026-0367.html)
- [ZIP symlink escape: RUSTSEC-2026-0368](https://rustsec.org/advisories/RUSTSEC-2026-0368.html)
- [ZIP permissions through symlinks: RUSTSEC-2026-0369](https://rustsec.org/advisories/RUSTSEC-2026-0369.html)

- [crypto-bigint timing variability: RUSTSEC-2026-0387](https://github.com/RustSec/advisory-db/blob/main/crates/crypto-bigint/RUSTSEC-2026-0387.md)
- [Upstream crypto-bigint 0.7.6 fixes](https://github.com/RustCrypto/crypto-bigint/pull/1365)
