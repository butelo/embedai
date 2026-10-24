# Why embedai exists

## An agent is mostly a loop

A useful agent does not necessarily require a framework, a vector database, a plugin protocol, or a daemon. At its core, it is a conversation array and a loop:

1. Send context and tool descriptions to a model.
2. If the model returns function calls, execute them locally.
3. Append the calls and their outputs to context.
4. Repeat until the model returns an answer.

`embedai` keeps that mechanism visible. The code is the explanation, and the small size is a feature. This follows the spirit of [You Should Write An Agent](https://fly.io/blog/everyone-write-an-agent/).

## The source is the product

A prebuilt agent encourages somebody else's defaults: their model, tools, instructions, and trust decisions. A source-first harness starts with different questions:

- What machine needs help?
- What is the smallest set of tools this task requires?
- Which model endpoint is appropriate?
- How long should the credential live?
- What should be removed when the task ends?

The resulting binary is an instance of your decisions, not the product being distributed.

## Ephemeral by default

The intended lifecycle is:

```text
clone → customize → build → transfer → work → delete → revoke
```

This is useful for a temporary maintenance task, an experiment in a lab, or a remote machine that should not retain a development environment. Ephemerality reduces setup and leftover state, but it does not make credentials or shell access harmless.

## One file on the target

Cross-compilation moves complexity to the builder. A static musl executable can run on a compatible Linux machine without a language runtime, package installation, or TLS library setup. This is particularly helpful on small ARM boards and minimally provisioned servers.

Portability means one artifact per OS/architecture—not one magical binary for every machine.

## Hackability over extensibility

Plugin interfaces are valuable for large, stable ecosystems. They can be unnecessary overhead for a personal tool. Here, adding a capability means editing a schema and a Rust function. Removing a dangerous capability means deleting it from the tool list.

The project should remain easy to copy, read, and fork. A feature is not automatically an improvement if it makes the harness harder to understand.

## Trust is the hard part

Embedding a key solves deployment, not secrecy. Giving a model shell access solves flexibility, not safety. The harness therefore favors explicit trade-offs over claims of security:

- Embedded credentials are recoverable.
- Tool calls execute with the user's operating-system permissions.
- Tool output and prompt content can contain prompt injection.
- Timeouts and output limits are reliability controls, not sandboxing.
- The safest tool is the one not included.

Use the idea where those trade-offs are acceptable. Change the idea where they are not.

## Non-goals

`embedai` does not aim to become a hosted platform, a general plugin ecosystem, a multi-tenant service, or an autonomous long-running system. It is a small starting point for people who want to understand and own their agent loop.
