# Approved dependency ledger

The approved direct dependencies are `serde` (with `derive`), `serde_json` and `rustix`.
The visual terminal crate and its `ratatui`/`crossterm` pair were removed with
the terminal app in 0.0.16 (#776); no other crate is approved.

These registry name/version/licence/checksum facts are an independent approval
record, not generated from the lockfile during a test. The tests compare every
locked external crate with this record and check dependency direction using
Cargo metadata. Changing an approved dependency still needs the owner's approval.
Every row uses `registry+https://github.com/rust-lang/crates.io-index`.

| Crate | Version | Licence | Kind | Required by | Checksum (`Cargo.lock`) |
| --- | --- | --- | --- | --- | --- |
| `bitflags` | `2.13.2` | MIT OR Apache-2.0 | transitive | `rustix` | `3ded4057c258ba199e2d26386d3af3780957ecaee6c4ef4041c6b4b8b97c0b06` |
| `errno` | `0.3.15` | MIT OR Apache-2.0 | transitive | `rustix` | `e35d6e78f4cebb90e51d543db8fe41528851d6deeef6ef21ebd4bfdc1e784776` |
| `itoa` | `1.0.18` | MIT OR Apache-2.0 | transitive | `serde_json` | `8f42a60cbdf9a97f5d2305f08a87dc4e09308d1276d28c869c684d7777685682` |
| `libc` | `0.2.191` | MIT OR Apache-2.0 | transitive | `rustix`, `errno` | `65cd87c9a6d60536580b316ad985726104730a5151c99845f2490428ef91240a` |
| `linux-raw-sys` | `0.12.1` | Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT | transitive | `rustix` | `32a66949e030da00e8c7d4434b251670a91556f4144941d37452769c25d58a53` |
| `memchr` | `2.8.3` | Unlicense OR MIT | transitive | `serde_json` | `cf8baf1c55e62ffcace7a9f06f4bd9cd3f0c4beb022d3b367256b91b87513d98` |
| `proc-macro2` | `1.0.107` | MIT OR Apache-2.0 | transitive | `quote`, `serde_derive`, `syn` | `985e7ec9bb745e6ce6535b544d84d6cd6f7ad8bd711c398938ae983b91a766d9` |
| `quote` | `1.0.47` | MIT OR Apache-2.0 | transitive | `serde_derive`, `syn` | `1fbf4db142a473a8d80c26bbf18454ed458bf8d26c8219c331daecfdbd079001` |
| `rustix` | `1.1.6` | Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT | direct | `openwiz-cli` | `e5e76742e5294153d4dde1207dbec5bdd81eef6fabdc0d2118babadabfd858aa` |
| `serde` | `1.0.229` | MIT OR Apache-2.0 | direct | `openwiz-core`, `openwiz-adapter-cloudroom`, `openwiz-cli`, `openwiz-tests`, `serde_json` | `4148590afebada386688f18773da617792bf2ef03ffc1e4cbd2b1d45b023e0ba` |
| `serde_core` | `1.0.229` | MIT OR Apache-2.0 | transitive | `serde`, `serde_json` | `67dca2c9c51e58a4791a4b1ed58308b39c64224d349a935ab5039aa360942a48` |
| `serde_derive` | `1.0.229` | MIT OR Apache-2.0 | transitive | `serde`, `serde_core` | `e7a5d71263a5a7d47b41f6b3f06ba276f10cc18b0931f1799f710578e2309348` |
| `serde_json` | `1.0.154` | MIT OR Apache-2.0 | direct | `openwiz-adapter-cloudroom`, `openwiz-cli`, `openwiz-tests` | `e7e9cc8b1b85264074fbcc02a88680c4096b1e47df8f739dceb03bf482f04bd6` |
| `syn` | `3.0.9` | MIT OR Apache-2.0 | transitive | `serde_derive` | `d78c8dee4c7bf0e14673097256fed6142ce9d3b85a408189d07482442145823b` |
| `unicode-ident` | `1.0.27` | (MIT OR Apache-2.0) AND Unicode-3.0 | transitive | `proc-macro2`, `syn` | `a2c754d6c33795a1c324727428e5a7dedb5b06195f9890bdbcba760d3e246563` |
| `windows-link` | `0.2.1` | MIT OR Apache-2.0 | transitive | `windows-sys` | `f0805222e57f7521d6a62e36fa9163bc891acd422f971defe97d64e70d0a4fe5` |
| `windows-sys` | `0.61.2` | MIT OR Apache-2.0 | transitive | `rustix`, `errno` | `ae137229bcbd6cdf0f7b80a31df61766145077ddf49416a728b02cb3921ff3fc` |
| `zmij` | `1.0.23` | MIT | transitive | `serde_json` | `29666d0abbfad1e3dc4dcf6144730dd3a3ab225bbbdac83319345b1b44ccfc1b` |
