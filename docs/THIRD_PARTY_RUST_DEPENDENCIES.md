# Third-Party Rust Dependencies

Generated from `cargo metadata` on 18 November 2026. **Status of every entry: unevaluated** — licence strings are as declared by each crate and have not been reviewed by counsel; no Dependency Evaluation Record exists yet (Decision Register, amendment A-03-5). The list includes build-time and platform-specific crates that may not be linked on every target.

Packages: 102 non-workspace packages, as of `cargo metadata` on 20 November 2026 (all targets). The direct table below lists 10 of the 16 direct non-workspace dependencies of `nf-server`; the rest appear in the dependency tables of the metadata graph.

## Declared licences

| Licence expression | Crates |
|---|---|
| MIT OR Apache-2.0 | 57 |
| MIT | 21 |
| Apache-2.0 OR MIT | 10 |
| MIT/Apache-2.0 | 4 |
| Zlib | 1 |
| MIT AND BSD-3-Clause | 1 |
| Unlicense OR MIT | 1 |
| MIT OR Apache-2.0 OR LGPL-2.1-or-later | 1 |
| Apache-2.0 OR BSL-1.0 | 1 |
| Apache-2.0 | 1 |
| (MIT OR Apache-2.0) AND Unicode-3.0 | 1 |
| Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT | 1 |

## Entries needing attention (copyleft, weak copyleft or undeclared)

| Crate | Version | Licence |
|---|---|---|
| r-efi | 6.0.0 | MIT OR Apache-2.0 OR LGPL-2.1-or-later |

Shipped notices: `packaging/THIRD_PARTY_NOTICES.txt` lists the 96 distinct crates in the dependency graph of the release build of `nf-server` for `x86_64-unknown-linux-gnu` and `x86_64-pc-windows-gnu`, including build-time and procedural-macro crates that are not linked into the shipped binary (cargo-about 0.9.3, accepted licences in `packaging/about.toml`), with the full licence text of each. For a dual-licensed crate, the licence shown is the one elected via `packaging/about.toml`. `tests/notices_check.sh` fails if the notices list does not match `cargo tree -e normal,build` for both targets. The tables below cover the whole metadata graph (102 non-workspace packages, all targets, including dev-only and wasm/UEFI-only crates) and remain unevaluated.

Bundled native code: `rusqlite` (feature `bundled`) compiles SQLite from source; SQLite is in the public domain according to its authors — to be confirmed in the evaluation record.

## Direct dependencies

| Crate | Version | Licence |
|---|---|---|
| aes-gcm | 0.11.1 | Apache-2.0 OR MIT |
| argon2 | 0.6.0 | MIT OR Apache-2.0 |
| axum | 0.8.9 | MIT |
| hex | 0.4.3 | MIT OR Apache-2.0 |
| rand | 0.10.4 | MIT OR Apache-2.0 |
| rusqlite | 0.43.0 | MIT |
| serde | 1.0.229 | MIT OR Apache-2.0 |
| serde_json | 1.0.154 | MIT OR Apache-2.0 |
| sha2 | 0.11.1 | MIT OR Apache-2.0 |
| tokio | 1.54.1 | MIT |
