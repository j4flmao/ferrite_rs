# Contributing to Ferrite

Thanks for taking the time to contribute. This guide covers how to set up the
project, the checks your change must pass, and how to get it merged.

## Code of conduct

Be respectful and constructive. Harassment, discrimination, and hostile
behaviour are not tolerated in issues, pull requests, or any project space.

## Prerequisites

- A **stable Rust toolchain** with the `rustfmt` and `clippy` components:

  ```bash
  rustup toolchain install stable --component rustfmt clippy
  ```

- Optional tooling used by some workflows:
  - [`cargo-watch`](https://crates.io/crates/cargo-watch) for `fr up`
  - [`mdbook`](https://crates.io/crates/mdbook) for `fr docs`
  - [`cargo-llvm-cov`](https://crates.io/crates/cargo-llvm-cov) for
    `fr test --coverage`
  - Docker, if you want to run examples that need Redis or PostgreSQL

## Getting started

```bash
git clone https://github.com/j4flmao/ferrite_rs
cd ferrite
cargo build --workspace
cargo test --workspace
```

Run a reference application to exercise the framework end to end:

```bash
cargo run -p micro-gateway
```

## Checks to run before pushing

CI runs exactly the commands below. Run them locally so your pull request lands
green.

```bash
# 1. Formatting
cargo fmt --all -- --check

# 2. Lints (framework crates are held to warnings-as-errors)
cargo clippy --workspace --all-targets --exclude 'micro-*' --exclude 'mono-*' -- -D warnings

# 3. Build and test everything, including examples
cargo build --workspace
cargo test --workspace

# 4. Documentation builds without warnings
cargo doc --workspace --no-deps
```

`cargo fmt --all` applies the formatting automatically. The example
applications under `examples/` are compiled and tested but are intentionally
excluded from the strict lint gate.

## Repository layout

```text
crates/
  ferrite-framework/   entry point that re-exports the core crates
  fr-core/        kernel: DI container, module graph, lifecycle
  ferrite-macros/      #[module], #[controller], #[injectable], ...
  ferrite-http/        Axum transport, routing, request pipeline
  ferrite-config/      layered .env + ferrite.toml configuration
  ferrite-orm*/        ORM abstraction and adapters (sqlx, diesel, sea)
  ferrite-*/           ecosystem crates (auth, cache, queue, ws, grpc, ...)
  fr-cli/              the `fr` command-line tool and project templates
examples/              runnable monolith and microservice applications (not published)
```

## Adding an ecosystem crate

1. Create `crates/ferrite-<name>/` with a `Cargo.toml` that inherits
   `version.workspace`, `edition.workspace`, and `license.workspace`.
2. Register it in the root `Cargo.toml`:
   - add the path to `[workspace] members`
   - add a `ferrite-<name>` entry to `[workspace.dependencies]`
3. If it should be installable via the CLI, add a short name to the mapping in
   `crates/fr-cli/src/commands/add.rs`.
4. Add a row to the ecosystem table in `README.md` and
   `09-ECOSYSTEM-PACKAGES.md`.
5. Add tests under `tests/` or inline `#[cfg(test)]` modules.

## Coding guidelines

- Format with `rustfmt`; keep clippy clean (warnings are errors for framework
  crates).
- Prefer the existing abstractions and patterns over new dependencies. Any new
  dependency must be added to `[workspace.dependencies]`.
- Keep public APIs documented with doc comments; intra-doc links must resolve
  (CI builds docs and rustdoc warnings are treated as noise to fix).
- Add tests for behaviour changes. Bug fixes should include a regression test.
- Update `README.md` and the relevant guide documents when you change
  user-facing behaviour.

## Commits and pull requests

- Keep pull requests focused on a single concern; small PRs review faster.
- Write clear commit messages. A conventional prefix is appreciated but not
  required (`feat:`, `fix:`, `docs:`, `refactor:`, `test:`, `chore:`).
- Reference the issue your PR closes (for example, `Closes #123`).
- Do not include generated artifacts or secrets. `Cargo.lock` is committed and
  should be updated deliberately.
- A path-based labeler applies labels such as `framework`, `cli`, `examples`,
  `docs`, `ci`, and `dependencies` automatically.
- Titles that look like work in progress (for example, `WIP:` or `[Draft]`) are
  rejected until the PR is ready for review.

Use the issue templates for bug reports and feature requests, and include
reproduction steps.

## License

By contributing, you agree that your contributions are licensed under the
[MIT License](LICENSE), without additional terms or conditions.
