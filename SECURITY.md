# Security

## Embedded credentials are not secret

Anyone who obtains a binary can potentially recover its API key through static strings, memory inspection, or reverse engineering. Obfuscation does not change this fundamental property.

For the intended personal, short-lived workflow:

- Create a dedicated key with minimum permissions.
- Set the smallest practical spending and rate limits.
- Do not commit the key or upload a binary containing it.
- Remove artifacts after use and revoke/rotate the key.
- Treat unexpected endpoint usage as a possible credential leak.

For shared or long-lived deployments, put the upstream credential behind a server-side proxy. Give each device a revocable, limited credential instead of embedding the provider's real key.

## Tools are not sandboxed

`shell`, `write_file`, and the other tools operate with all permissions of the user running `embedai`. There are no approval prompts. A model can make mistakes, and fetched or file content can contain prompt injection.

Prefer a low-privilege account, a narrow tool set, and a disposable machine or container. Do not point the agent at sensitive data or production systems unless the consequences are understood and controlled. Command timeouts and output truncation do not form a security boundary.

## Command-line and build exposure

`build.sh --api-key ...` is convenient but arguments may appear in shell history and process listings. Container builds additionally pass the value into the build environment. Use only a disposable key and clear relevant history entries when appropriate.

Keep build directories private: release artifacts, dependency artifacts, and copied executables may contain the embedded key. `cargo clean` removes this project's build artifacts; it does not erase backups, filesystem snapshots, shell history, or copies made elsewhere.

## Reporting vulnerabilities

Do not include API keys, private endpoints, or sensitive device details in a public issue. Report vulnerabilities through GitHub's private vulnerability reporting when enabled, or open a minimal public issue requesting a private contact method.
