# Changelog

## [0.1.41](https://github.com/srobinson/fmm/compare/v0.1.40...v0.1.41) (2026-03-16)


### Features

* multi-language architecture with LanguageDescriptor, per-lang modules, and named imports ([#110](https://github.com/srobinson/fmm/issues/110)) ([b15d1fb](https://github.com/srobinson/fmm/commit/b15d1fb537e47ffa55f58cd06eacb09ff1ef7a81))

## [0.1.40](https://github.com/srobinson/fmm/compare/v0.1.39...v0.1.40) (2026-03-08)


### Bug Fixes

* surface version mismatch error instead of generic "No index found" ([72f9bb9](https://github.com/srobinson/fmm/commit/72f9bb9a7d81854ce81bb583e538cd419671ac6e))

## [0.1.39](https://github.com/srobinson/fmm/compare/v0.1.38...v0.1.39) (2026-03-08)


### Performance

* parallelize fmm generate for large codebases ([#107](https://github.com/srobinson/fmm/issues/107)) ([84f09f7](https://github.com/srobinson/fmm/commit/84f09f7816c1447a6a79c34a6a38237e4c10a507))

## [0.1.38](https://github.com/srobinson/fmm/compare/v0.1.37...v0.1.38) (2026-03-07)


### Bug Fixes

* detect stale index after fmm upgrade ([1dd32c1](https://github.com/srobinson/fmm/commit/1dd32c1ce4f53cd533e5e154e932c65356ac2666))


### Performance

* opt-level 3 and 64MB SQLite page cache ([3d3ab57](https://github.com/srobinson/fmm/commit/3d3ab574da83a0923d4dcbcf1f51e827635835b7))

## [0.1.37](https://github.com/srobinson/fmm/compare/v0.1.36...v0.1.37) (2026-03-07)

### Features

- TypeScript scale — make fmm work on 39k+ file codebases (#ALP-923) ([#104](https://github.com/srobinson/fmm/issues/104)) ([a5767e8](https://github.com/srobinson/fmm/commit/a5767e88339e9595036275dd8d5d0827980e9def))

## [0.1.36](https://github.com/srobinson/fmm/compare/v0.1.35...v0.1.36) (2026-03-07)

### Bug Fixes

- **glossary:** multi-line used_by format and truncate parameter (#ALP-919) ([15c34d1](https://github.com/srobinson/fmm/commit/15c34d1da1ae85ebf5f4191f5dacae5b9622d080))

## [0.1.35](https://github.com/srobinson/fmm/compare/v0.1.34...v0.1.35) (2026-03-07)

### Features

- SQLite manifest store — replace per-file YAML sidecars (#ALP-912) ([#101](https://github.com/srobinson/fmm/issues/101)) ([dabc5f3](https://github.com/srobinson/fmm/commit/dabc5f327de8b6b85a5e4ca5e06aa3a72bf136e4))

## [0.1.34](https://github.com/srobinson/fmm/compare/v0.1.33...v0.1.34) (2026-03-07)

### Features

- non-exported top-level functions in outline and read_symbol (#ALP-909) ([#99](https://github.com/srobinson/fmm/issues/99)) ([b8c9783](https://github.com/srobinson/fmm/commit/b8c97831bcadeeb11c59ad54d7676c142aca56ba))

## [0.1.33](https://github.com/srobinson/fmm/compare/v0.1.32...v0.1.33) (2026-03-07)

### Bug Fixes

- **glossary:** cross-package bare specifier matching + disclosure typo (#ALP-905) ([#97](https://github.com/srobinson/fmm/issues/97)) ([65015b4](https://github.com/srobinson/fmm/commit/65015b43964e1b1dacc97c106b3b820e79ea5416))

## [0.1.32](https://github.com/srobinson/fmm/compare/v0.1.31...v0.1.32) (2026-03-07)

### Features

- **search:** named-import call-site discovery in fmm_search ([#95](https://github.com/srobinson/fmm/issues/95)) ([453f293](https://github.com/srobinson/fmm/commit/453f29301536579903bd2e1ed3f1a8501ff5a86c))

## [0.1.31](https://github.com/srobinson/fmm/compare/v0.1.30...v0.1.31) (2026-03-07)

### Bug Fixes

- **search:** fmm_search bug fixes from TanStack user feedback ([#90](https://github.com/srobinson/fmm/issues/90)) ([dd2af2a](https://github.com/srobinson/fmm/commit/dd2af2ab8d80a9a422e5456d74d471d1c899ad5a))

## [0.1.30](https://github.com/srobinson/fmm/compare/v0.1.29...v0.1.30) (2026-03-06)

### Features

- **docs:** tools.toml single source of truth — build-time doc generation + skill sync ([#88](https://github.com/srobinson/fmm/issues/88)) ([ca83b3f](https://github.com/srobinson/fmm/commit/ca83b3f782817024dd0f3ee68ba15e1bf8f3b70d))

## [0.1.29](https://github.com/srobinson/fmm/compare/v0.1.28...v0.1.29) (2026-03-06)

### Features

- **glossary:** layered call-site precision — named import tracking + used_by filtering ([#85](https://github.com/srobinson/fmm/issues/85)) ([678d16b](https://github.com/srobinson/fmm/commit/678d16b2b4f9a89fcbc4a9753fb73126c67925d7))

## [0.1.28](https://github.com/srobinson/fmm/compare/v0.1.27...v0.1.28) (2026-03-06)

### Features

- **resolver:** cross-package import resolution for accurate downstream graph ([#83](https://github.com/srobinson/fmm/issues/83)) ([025a8c0](https://github.com/srobinson/fmm/commit/025a8c0af807786ec021e5ef11d3ef1ab4bb43ec))

## [0.1.27](https://github.com/srobinson/fmm/compare/v0.1.26...v0.1.27) (2026-03-06)

### Features

- **mcp+cli:** Round 5 evaluation improvements — 5 fixes + 13 enhancements ([#81](https://github.com/srobinson/fmm/issues/81)) ([e1e0953](https://github.com/srobinson/fmm/commit/e1e095379fa1773c9d5236773b620ae03cf82492))

## [0.1.26](https://github.com/srobinson/fmm/compare/v0.1.25...v0.1.26) (2026-03-06)

### Features

- **mcp+cli:** Round 4 evaluation improvements — 5 bug fixes + 4 enhancements ([#79](https://github.com/srobinson/fmm/issues/79)) ([044ce50](https://github.com/srobinson/fmm/commit/044ce50b6819416bfd67d0378cf916c7797557e5))

## [0.1.25](https://github.com/srobinson/fmm/compare/v0.1.24...v0.1.25) (2026-03-06)

### Features

- **mcp+cli:** Round 3 evaluation improvements — 12 issues ([#77](https://github.com/srobinson/fmm/issues/77)) ([980e3a7](https://github.com/srobinson/fmm/commit/980e3a774c9343f8c491b541fd9b7e1ff11fe726))

## [0.1.24](https://github.com/srobinson/fmm/compare/v0.1.23...v0.1.24) (2026-03-05)

### Features

- **cli:** full CLI parity with MCP — lookup, read, deps, outline, ls, exports ([#75](https://github.com/srobinson/fmm/issues/75)) ([70d21d9](https://github.com/srobinson/fmm/commit/70d21d9435f5f8f3038ea8350fdd5bfcaccef6a0))

## [0.1.23](https://github.com/srobinson/fmm/compare/v0.1.22...v0.1.23) (2026-03-05)

### Bug Fixes

- **manifest:** resolve two local_deps gaps in fmm_dependency_graph ([c79532a](https://github.com/srobinson/fmm/commit/c79532a4d7b1760c23d14e34ee4bb1b2973376ae))

## [0.1.22](https://github.com/srobinson/fmm/compare/v0.1.21...v0.1.22) (2026-03-05)

### Performance

- **manifest:** O(1) reverse dependency index for downstream lookups ([519b453](https://github.com/srobinson/fmm/commit/519b453ff6cb9a8871501fe431cbdb8a8bcc95cd))

## [0.1.21](https://github.com/srobinson/fmm/compare/v0.1.20...v0.1.21) (2026-03-05)

### Features

- MCP improvements and parser fixes — ALP-798 to ALP-803 ([#69](https://github.com/srobinson/fmm/issues/69)) ([3dac27e](https://github.com/srobinson/fmm/commit/3dac27ef6a95f1feb3bc5775889822531e6f4766))

## [0.1.20](https://github.com/srobinson/fmm/compare/v0.1.19...v0.1.20) (2026-03-05)

### Bug Fixes

- parser import classification — ghost entries and missing local deps (ALP-792) ([#66](https://github.com/srobinson/fmm/issues/66)) ([5e524ec](https://github.com/srobinson/fmm/commit/5e524ecd3430e910dc2f8f0e315a6fde9079d8af))

## [0.1.19](https://github.com/srobinson/fmm/compare/v0.1.18...v0.1.19) (2026-03-05)

### Features

- MCP tool improvements — pagination, truncate, transitive graph, call-site precision, combined filters (ALP-791) ([#64](https://github.com/srobinson/fmm/issues/64)) ([acf53e0](https://github.com/srobinson/fmm/commit/acf53e0d9429fcfeefda100b9caaa481113ac43b))

## [0.1.18](https://github.com/srobinson/fmm/compare/v0.1.17...v0.1.18) (2026-03-05)

### Features

- expose class methods in fmm_lookup_export, fmm_list_exports, fmm_glossary (ALP-777) ([#62](https://github.com/srobinson/fmm/issues/62)) ([ee6136a](https://github.com/srobinson/fmm/commit/ee6136aae894fb5b200ea2161aeaa99bdec2759c))

## [0.1.17](https://github.com/srobinson/fmm/compare/v0.1.16...v0.1.17) (2026-03-05)

### Features

- public method indexing — dotted symbol navigation (ClassName.method) (ALP-764) ([#60](https://github.com/srobinson/fmm/issues/60)) ([3b4db1d](https://github.com/srobinson/fmm/commit/3b4db1d895e7bffc2be6000215c82df6bb1e44a9))

## [0.1.16](https://github.com/srobinson/fmm/compare/v0.1.15...v0.1.16) (2026-03-05)

### Features

- Rust parser production readiness — pub use, macro_export, wildcard deps (ALP-773) ([#58](https://github.com/srobinson/fmm/issues/58)) ([7598ddb](https://github.com/srobinson/fmm/commit/7598ddb3612661dc55774573d3bc41de310683fa))

## [0.1.15](https://github.com/srobinson/fmm/compare/v0.1.14...v0.1.15) (2026-03-05)

### Features

- TypeScript parser hardening and tool improvements (ALP-748) ([#56](https://github.com/srobinson/fmm/issues/56)) ([31904f3](https://github.com/srobinson/fmm/commit/31904f365bccd60c7799e9628757f4e2f892c456))

## [0.1.14](https://github.com/srobinson/fmm/compare/v0.1.13...v0.1.14) (2026-03-05)

### Features

- TypeScript parser hardening ([#54](https://github.com/srobinson/fmm/issues/54)) ([26ef912](https://github.com/srobinson/fmm/commit/26ef91282959bd504e88c9b9dfd2bf80dda257a3))

## [0.1.13](https://github.com/srobinson/fmm/compare/v0.1.12...v0.1.13) (2026-03-05)

### Features

- glossary feature -- symbol-level impact analysis ([#52](https://github.com/srobinson/fmm/issues/52)) ([b6c4669](https://github.com/srobinson/fmm/commit/b6c46694d9f08c84f041cc90d7b26e353b9442cd))

## [0.1.12](https://github.com/srobinson/fmm/compare/v0.1.11...v0.1.12) (2026-03-05)

### Features

- field report MCP hardening, dependency graph, and parser improvements ([#50](https://github.com/srobinson/fmm/issues/50)) ([18001ff](https://github.com/srobinson/fmm/commit/18001ffa27fd9b02536410c09346493d6a4185ca))
- support decorated Python definitions and add --force generate flag ([52e5309](https://github.com/srobinson/fmm/commit/52e5309702452d3fb071206eecd3805747da103e))

## [0.1.11](https://github.com/srobinson/fmm/compare/v0.1.10...v0.1.11) (2026-03-04)

### Features

- unify MCP output to sidecar YAML and index Rust binary crates ([4b4d6af](https://github.com/srobinson/fmm/commit/4b4d6afebb581013acac4af0975301e6be0bdd93))

## [0.1.10](https://github.com/srobinson/fmm/compare/v0.1.9...v0.1.10) (2026-03-04)

### Bug Fixes

- guard fromJSON against empty release-please pr output ([2a0c8d8](https://github.com/srobinson/fmm/commit/2a0c8d83a89c700608e29b3e547960bf33347ac3))

## [0.1.9](https://github.com/srobinson/fmm/compare/v0.1.8...v0.1.9) (2026-03-04)

### Bug Fixes

- automated release pipeline and sidecar discovery ([d6952fe](https://github.com/srobinson/fmm/commit/d6952fe6183b798887041814849d0b63682e7c99))
- extract PR number from release-please JSON output ([e953b99](https://github.com/srobinson/fmm/commit/e953b9917abb345760f92d9ed9f38570c4db0328))

## [0.1.8](https://github.com/srobinson/fmm/compare/v0.1.7...v0.1.8) (2026-03-04)

### Bug Fixes

- resolve conflict markers in fixture sidecars ([fba0703](https://github.com/srobinson/fmm/commit/fba07033709be3bbe025dd14e66019673781d0c4))

## [0.1.7](https://github.com/srobinson/fmm/compare/v0.1.6...v0.1.7) (2026-03-04)

### Bug Fixes

- parser correctness issues and deterministic config ordering ([#43](https://github.com/srobinson/fmm/issues/43)) ([97d6370](https://github.com/srobinson/fmm/commit/97d63700f88114356d0bfb642f5c27d62458ea2a))

## [0.1.6](https://github.com/srobinson/fmm/compare/v0.1.5...v0.1.6) (2026-03-04)

### Features

- add language parsers for PHP, C, Zig, Lua, Scala ([#40](https://github.com/srobinson/fmm/issues/40)) ([fc13ad8](https://github.com/srobinson/fmm/commit/fc13ad849a0e678774047c8667a4ff119e78da4c))
- add language parsers for Swift, Kotlin, Dart, Elixir ([#42](https://github.com/srobinson/fmm/issues/42)) ([30b29e4](https://github.com/srobinson/fmm/commit/30b29e4a9a07659770c4e672125fbd152b2f8207))

## [0.1.5](https://github.com/srobinson/fmm/compare/v0.1.4...v0.1.5) (2026-03-03)

### Bug Fixes

- use parent node for Rust impl method line ranges, sort exports by line number ([#38](https://github.com/srobinson/fmm/issues/38)) ([d6e04d0](https://github.com/srobinson/fmm/commit/d6e04d05e7a8fabe79f8b1eccb98c31921d76272))

## [0.1.4](https://github.com/srobinson/fmm/compare/v0.1.3...v0.1.4) (2026-03-03)

### Features

- make .claude/ dir creation opt-in in fmm init ([#36](https://github.com/srobinson/fmm/issues/36)) ([cd107a5](https://github.com/srobinson/fmm/commit/cd107a52af2b53421f6a62be7a0903b0401e5441))

## [0.1.3](https://github.com/srobinson/fmm/compare/v0.1.2...v0.1.3) (2026-03-03)

### Features

- support multiple paths in generate, validate, and clean commands ([87478c3](https://github.com/srobinson/fmm/commit/87478c3ad70354614ffa16203b7cc9d091278787))

## [0.1.2](https://github.com/srobinson/fmm/compare/v0.1.1...v0.1.2) (2026-02-14)

### Features

- world-class CLI help system ([#27](https://github.com/srobinson/fmm/issues/27)) ([dffbacf](https://github.com/srobinson/fmm/commit/dffbacf1d7105f4268c987b09f11f8e6c1e088f6))

### Bug Fixes

- remove crates.io publish from release pipeline ([#25](https://github.com/srobinson/fmm/issues/25)) ([39125dc](https://github.com/srobinson/fmm/commit/39125dc8f7f8f68370df1058a5a8ea20c4c058de))

## [0.1.1](https://github.com/srobinson/fmm/compare/v0.1.0...v0.1.1) (2026-02-14)

### Features

- Add search CLI, MCP server, and Claude integration ([43c9e3b](https://github.com/srobinson/fmm/commit/43c9e3b3fc2d851af48f1e0aae3cfb33d8397f4b))
- automated release pipeline with npm distribution ([4b829cc](https://github.com/srobinson/fmm/commit/4b829cc2571a2e8cc16e754b8d6c375b1f40c5a9))

### Bug Fixes

- Address clippy warnings ([9bf789d](https://github.com/srobinson/fmm/commit/9bf789d1129bee08e98126441f8a4ae0faaa08b4))
- Apply cargo fmt to report.rs ([6849220](https://github.com/srobinson/fmm/commit/684922047047cc69279bb163a6291457271e4563))
- cargo fmt trailing whitespace ([88a12df](https://github.com/srobinson/fmm/commit/88a12df30c2003293df4d5e7935782b40b80a704))
- reference resolution — stem matching + fallback cap ([c4c89f4](https://github.com/srobinson/fmm/commit/c4c89f47d6657b804dd76835bbaaf053f32c1149))
- remaining cargo fmt issue ([2a598be](https://github.com/srobinson/fmm/commit/2a598be7fadd9c2e1d02164f740e40f8559458ca))
