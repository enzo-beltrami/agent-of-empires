# Agent of Empires (personal fork)

A personal fork of [Agent of Empires](https://github.com/agent-of-empires/agent-of-empires),
a session manager for AI coding agents, trimmed to how it is used here: the
`aoe serve` daemon with its browser dashboard and the coordinator.

Each agent runs in its own tmux session, so it keeps running when the browser
closes or the daemon restarts.

## Build and run

Requires tmux, Rust and Node.

```bash
cargo build --profile dev-release --features web
aoe serve
```

Local builds, tests and the development loop are in
[docs/development.md](docs/development.md). The HTTP and MCP API is in
[docs/api.md](docs/api.md).

## License

MIT License. See [LICENSE](LICENSE).
