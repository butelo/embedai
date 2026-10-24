# Contributing

Thank you for helping improve embedai.

## Project direction

The project is a small, source-first, disposable agent harness. Prefer changes that preserve readability and independence from heavyweight frameworks. Good contributions include:

- Correctness and reliability fixes
- Better cross-compilation support
- Small, broadly useful tools
- Safer defaults that do not pretend to be a sandbox
- Tests and documentation

Large plugin systems, hosted-service features, and persistent agent infrastructure are likely out of scope; a fork may be a better home for those ideas.

## Development

Use a current stable Rust toolchain and run:

```sh
cargo fmt --check
cargo test --locked
cargo clippy --locked --all-targets -- -D warnings
sh -n build.sh
```

Tests must not require real AI credentials or paid model calls. Use the existing mock Responses API pattern for agent-loop tests.

## Security and credentials

Never commit real credentials, binaries containing credentials, private endpoint details, or personal SSH keys. Use placeholder values in examples and tests. Test keys such as `test-key` are sufficient for compilation.

## Pull requests

Keep changes focused and explain the problem being solved. Update documentation when behavior or builder options change. By contributing, you agree that your contribution is licensed under the project's MIT license.
