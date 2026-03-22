# fmm (frontmatter-matters)

Code structural intelligence for AI agents. Rust workspace with `fmm-core` (library) and `fmm` (CLI).

## Commands

Use `just` for all workflows. Run `just` to see available recipes.

```
just test      # nextest + doctests (NOT cargo test)
just check     # fmt + clippy
just ci        # check + test + build
```

**Do not run `cargo test`**. The config tests mutate environment variables and require process-per-test isolation that only nextest provides. `cargo test` will produce spurious failures.
