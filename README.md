# embedai

**Build a tiny agent. Carry it to a machine. Do the work. Delete it.**

`embedai` is a source-first recipe for a personal, disposable AI agent harness. You choose the endpoint, model, credentials, and tools; the build produces one terminal executable for the machine where you need it. There is no service to install, account to create, plugin system, or prebuilt agent containing somebody else's credentials.

The project is intentionally small enough to read and modify. It implements the OpenAI-compatible [Responses API](https://platform.openai.com/docs/api-reference/responses) tool-calling loop described in [You Should Write An Agent](https://fly.io/blog/everyone-write-an-agent/): send context, execute returned function calls, append their outputs, and repeat until the model answers.

## Philosophy

**The source is the product—not the binary.**

- **Personal:** build with your own endpoint, model, key, and purpose.
- **Ephemeral:** use a short-lived credential, do the job, remove the artifact, revoke the key.
- **Portable:** one executable, including static Linux builds for small or remote machines.
- **Hackable:** ordinary Rust and JSON; add a tool by editing one file.
- **Transparent:** no hidden agent framework, MCP server, background daemon, or persistent context.
- **Disposable credentials, not disposable caution:** anyone holding the binary can potentially extract its key or use its tools.

Read [the design philosophy](docs/PHILOSOPHY.md) and [security guidance](SECURITY.md) before giving an agent access to a machine.

## What it is useful for

- One-off diagnostics or maintenance on a remote Linux server
- A temporary assistant on a Raspberry Pi, lab machine, or rescue environment
- Machines where installing Python, Node.js, and dependency trees is undesirable
- Learning how little machinery an LLM tool-calling loop actually needs
- Prototyping a narrow, custom agent with your own tools and instructions
- A starting point for a purpose-built internal harness

## What it is not

- A hosted service or multi-user agent platform
- A sandbox: shell and file tools have the permissions of the invoking user
- A secure way to distribute a valuable secret
- A replacement for approval workflows around destructive operations
- A persistent assistant with long-term memory
- A universal executable: build one artifact per operating system/architecture

## Quick start

Requirements for a native build: a current stable Rust toolchain. For containerized cross-compilation, use Docker **or** Podman instead. Using a JSON build config also requires [`jq`](https://jqlang.org/).

```sh
git clone https://github.com/butelo/embedai.git
cd embedai

./build.sh \
  --endpoint https://api.openai.com/v1 \
  --api-key sk-your-short-lived-key \
  --model gpt-5-mini \
  --output embedai

./embedai
```

The executable has the endpoint, API key, and model embedded. It does not need those environment variables when it runs.

### Build from a config file

Pass JSON with `--config` instead of putting every value on the command line. Config files are parsed with `jq`; normal builds without `--config` do not need it.

```sh
cp build.example.json build.json
# Edit build.json, then run:
./build.sh --config build.json
```

```json
{
  "endpoint": "https://api.openai.com/v1",
  "api_key": "sk-your-short-lived-key",
  "model": "gpt-5-mini",
  "target": "aarch64-unknown-linux-musl",
  "podman": true,
  "output": "embedai-linux-arm64"
}
```

The supported keys are `endpoint`, `api_key`, `model`, `target`, `output`, `docker`, and `podman`. The first three are required. `target` and `output` are optional. Container options are optional booleans; do not set both to `true`.

Command-line options override config values, so you can reuse a file while changing a setting:

```sh
./build.sh --config build.json --model another-model --output embedai-test
```

The default `build.json` filename is git-ignored. Keep config files private if you use another filename.

> **Use a disposable key.** Command-line arguments can appear in shell history and process listings, and embedded keys can be recovered from binaries. Config files avoid putting the key in build command arguments, but still store it in plaintext. See [SECURITY.md](SECURITY.md).

## Cross-compile: build here, run there

For an ARM64 Linux server or 64-bit Raspberry Pi:

```sh
./build.sh \
  --endpoint https://your-endpoint.example/v1 \
  --api-key sk-your-short-lived-key \
  --model your-model \
  --target aarch64-unknown-linux-musl \
  --podman \
  --output embedai-linux-arm64
```

Use `--docker` instead of `--podman` if that is your container engine. Both paths use prebuilt musl toolchains; you do not need a local Linux cross-linker. The container is needed only to build, never to run the agent.

Copy the result with `scp`, serve it with any HTTP server, or put it on removable media:

```sh
scp embedai-linux-arm64 user@device:/tmp/embedai
ssh user@device /tmp/embedai
```

Or, on the device:

```sh
wget https://your-server.example/embedai-linux-arm64 -O embedai
chmod +x embedai
./embedai
```

Useful targets:

| Rust target | Typical machine |
| --- | --- |
| `x86_64-unknown-linux-musl` | Intel/AMD Linux, static |
| `aarch64-unknown-linux-musl` | ARM64 Linux, Raspberry Pi 3/4/5 64-bit, static |
| `armv7-unknown-linux-musleabihf` | Older 32-bit ARM Linux, static |
| `aarch64-apple-darwin` | Apple Silicon |
| `x86_64-apple-darwin` | Intel Mac |
| `x86_64-pc-windows-msvc` | Windows x64 |

Other Rust targets may work with an appropriate linker and container image. Run `./build.sh --help` for every builder option.

## Terminal usage

```text
> Check disk usage and explain anything unusual
  tool: shell {"command":"df -h"}
>>> ...
```

Commands inside the agent:

| Command | Effect |
| --- | --- |
| `/help` | Show terminal commands and available tools |
| `/clear` | Discard in-memory conversation context |
| `/quit` | Exit |

It also works in scripts:

```sh
printf 'Report the current directory\n/quit\n' | ./embedai
```

### Override the embedded endpoint or model

An embedded binary can be pointed elsewhere without rebuilding:

```sh
./embedai --endpoint http://localhost:8080/v1 --model local-model
# A complete Responses URL is accepted too:
./embedai -e http://localhost:8080/v1/responses -m local-model
```

The endpoint must implement `POST /responses` and Responses API function calling. A server offering only `/chat/completions` is not compatible.

Configuration precedence is:

1. CLI endpoint/model options
2. Runtime `OPENAI_BASE_URL`, `OPENAI_MODEL`, and `OPENAI_API_KEY`
3. Values embedded at build time
4. Defaults for endpoint and model

## Built-in tools

| Tool | Purpose |
| --- | --- |
| `shell` | Run a command through `sh` on Unix or `cmd` on Windows |
| `read_file` | Read a text file |
| `write_file` | Create or replace a text file |
| `list_dir` | List directory entries |
| `ping` | Use the system's `ping` executable |

There are deliberately no tool approval prompts. Commands have a 30-second default timeout, tool output is bounded, and the model loop stops after 24 turns by default. These limits reduce accidents; they do **not** provide a security boundary.

## Make it yours

The core is split into four small files:

| File | Responsibility |
| --- | --- |
| `src/agent.rs` | Responses API calls, context replay, and the agent loop |
| `src/tools.rs` | Tool JSON schemas and implementations |
| `src/config.rs` | Embedded and runtime configuration |
| `src/main.rs` | Terminal and command-line interface |

To add a tool, add its schema to `tools::definitions()` and its dispatcher/implementation in `tools::execute()`. Change `INSTRUCTIONS` in `src/agent.rs` to give your harness a specific job. Remove `shell` if you want a narrower agent. This is meant to be edited, not extended through a plugin API.

## Build without the script

The script only forwards values to Cargo:

```sh
export OPENAI_API_KEY='sk-your-short-lived-key'
export OPENAI_BASE_URL='https://your-endpoint.example/v1'
export OPENAI_MODEL='your-model'
cargo build --release --locked
unset OPENAI_API_KEY OPENAI_BASE_URL OPENAI_MODEL
```

The artifact is `target/release/embedai`, or `target/<target-triple>/release/embedai` when cross-compiling. Rebuild after changing embedded values.

## Development

```sh
cargo test --locked
cargo clippy --locked --all-targets -- -D warnings
sh -n build.sh
```

Tests use local commands and a mock Responses API. No AI key or network model call is required. CI builds and tests source with placeholder credentials and never publishes binaries.

## Cleanup

When the task is complete:

1. Delete the binary from the target machine.
2. Delete local copies and build artifacts if they contain the credential (`cargo clean`).
3. Revoke or rotate the API key. Deleting a binary cannot revoke a copied key.

## Contributing

Ideas, bug reports, narrow tools, target-specific build improvements, and clearer documentation are welcome. See [CONTRIBUTING.md](CONTRIBUTING.md). The project favors small, inspectable changes over framework growth.

## License

MIT. Build what you need; take responsibility for what your agent can access.
