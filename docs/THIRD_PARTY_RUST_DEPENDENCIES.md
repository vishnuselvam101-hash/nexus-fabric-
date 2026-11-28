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

## Added by nf-nest (review round 1, 28 November 2026)

Crates added to Cargo.lock by the nesting crate `nf-nest` (jagua-rs placement engine, encoding_rs for the R12
Shift_JIS hand-off). The licence of each was read from `cargo metadata --locked` and checked against the rule
that GPL, LGPL, AGPL and undeclared licences are refused. None is refused: 31 crates, all permissive or weak
copyleft (MPL-2.0, `jagua-rs`, used unmodified, pinned `=2.0.0`). Status: declared licence, not yet reviewed by
counsel (Dependency Evaluation Record still pending).

Direct: `jagua-rs` 2.0.0 (MPL-2.0, https://github.com/JeroenGar/jagua-rs; full text in crates/nf-nest/LICENSE-MPL-2.0.txt),
`encoding_rs` 0.8.35 ((Apache-2.0 OR MIT) AND BSD-3-Clause).

| Crate | Version | Licence |
|---|---|---|
| anyhow | 1.0.105 | MIT OR Apache-2.0 |
| approx | 0.5.1 | Apache-2.0 |
| byteorder | 1.5.0 | Unlicense OR MIT |
| document-features | 0.2.12 | MIT OR Apache-2.0 |
| encoding_rs | 0.8.35 | (Apache-2.0 OR MIT) AND BSD-3-Clause |
| float-cmp | 0.10.0 | MIT |
| float_next_after | 2.0.0 | MIT |
| geo | 0.33.1 | MIT OR Apache-2.0 |
| geo-types | 0.7.21 | MIT OR Apache-2.0 |
| geographiclib-rs | 0.2.7 | MIT |
| hash32 | 0.3.1 | MIT OR Apache-2.0 |
| heapless | 0.8.0 | MIT OR Apache-2.0 |
| i_float | 1.16.0 | MIT |
| i_key_sort | 0.10.3 | MIT |
| i_overlay | 4.5.2 | MIT OR Apache-2.0 |
| i_shape | 1.18.0 | MIT |
| i_tree | 0.18.0 | MIT |
| itertools | 0.15.0 | MIT OR Apache-2.0 |
| jagua-rs | 2.0.0 | MPL-2.0 |
| js-sys | 0.3.107 | MIT OR Apache-2.0 |
| libm | 0.2.16 | MIT |
| litrs | 1.1.0 | MIT OR Apache-2.0 |
| ordered-float | 5.5.1 | MIT |
| rand_pcg | 0.10.3 | MIT OR Apache-2.0 |
| robust | 1.2.0 | MIT OR Apache-2.0 |
| rstar | 0.12.2 | MIT OR Apache-2.0 |
| sif-itree | 0.4.1 | MIT OR Apache-2.0 |
| slotmap | 1.1.1 | Zlib |
| stable_deref_trait | 1.2.1 | MIT OR Apache-2.0 |
| svg | 0.18.0 | Apache-2.0 OR MIT |
| web-time | 1.1.0 | MIT OR Apache-2.0 |
