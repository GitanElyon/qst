# Contributing to qst

Contributions to qst are welcome! This includes bug fixes, new features, documentation improvements, test coverage, and more. This guide covers the Rust launcher itself; scripts and the community catalog live in the `awesome-qst` repository and follow [its own contributing guide](https://github.com/GitanElyon/awesome-qst/blob/main/CONTRIBUTING.md).

## How to contribute

Either open a pull request with your changes or open an issue to discuss new ideas or problems. For large or experimental changes, consider opening an issue first so the direction can be agreed on before you invest time in the implementation.

- Branch off `main` and open a pull request against `main`.
- Keep pull requests focused on a single change. Separate refactors from feature work.
- The maintainer will review your changes and may ask for revisions.

## Development setup

qst requires Rust 1.85 or newer (edition 2024).

If you use Nix, the flake provides a development shell with everything needed:

```bash
nix develop
```

This includes `cargo`, `rustc`, `rustfmt`, `clippy`, and `rust-analyzer`. Otherwise, install the Rust toolchain with `rustup` and use `cargo` directly.

## Before submitting

Run the same checks that CI runs. All of them must pass:

```bash
cargo fmt --all --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
cargo build --release --locked
```

A quick manual smoke test of the TUI is also appreciated:

```bash
cargo run
```

## Code standards

- Follow `rustfmt` formatting; the CI `fmt` check must pass cleanly.
- Address every clippy warning. CI treats warnings as errors.
- Use the `log` crate (`debug!`, `info!`, `warn!`, `error!`) for diagnostics instead of printing to stdout or stderr. qst runs in raw mode; stray output corrupts the terminal.
- Keep tests next to the code in `#[cfg(test)] mod tests` blocks, one per module. Tests run on every push in CI.
- Update the `Unreleased` section of `CHANGELOG.md` with any user-visible changes, following the existing entry style.
- Prefer small, readable functions. The codebase values clarity over cleverness.

## Code architecture

A map of the source modules and notable implementation points lives in the [Code architecture](DOCS.md#code-architecture) section of `DOCS.md`.

The script protocol is documented in [API.md](API.md). If you change how scripts behave, update that contract and consider adding a matching example in the `awesome-qst` repo.

## Legal

By contributing to this repository, you agree that your contributions will be licensed under the MIT License. Please ensure that any code you submit is your original work or properly attributed if it includes third-party code.