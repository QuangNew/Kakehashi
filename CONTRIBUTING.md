# Contributing to Kakehashi

Kakehashi is a Windows desktop application derived from NexQ. Open issues and pull requests in [QuangNew/Kakehashi](https://github.com/QuangNew/Kakehashi) for changes specific to this project.

## Development setup

Use Windows 10/11 with WebView2, PowerShell 7, Node.js 24, the pnpm version declared in `package.json`, Rust/rustup, MinGW and Python. The desktop scripts prepare portable native build tools. See the [Windows development guide](docs/guides/development.vi.md) for build profiles, GPU support and data paths.

```powershell
git clone https://github.com/QuangNew/Kakehashi.git
cd Kakehashi
pnpm install --frozen-lockfile
pnpm models:setup
pnpm desktop:dev
```

Model setup downloads the local STT/translation models and runtime. Frontend tests do not need model weights or API keys.

## Changes and verification

Create a branch from `main`, follow the existing TypeScript/Rust conventions and describe the resulting behavior in the pull request. Keep provider configuration independent between translation and copilot. Preserve compatibility with existing meeting data and application settings.

Run the checks relevant to the change:

```powershell
pnpm build          # TypeScript and Vite
pnpm test           # Frontend tests
pnpm desktop:test   # Rust checks and unit tests with the desktop environment
pnpm desktop:build  # Windows desktop build
```

Use `pnpm models:smoke` for local model integration and `pnpm bench:realtime` for audio performance changes. State the hardware, models and measurement boundaries when reporting latency. Cloud providers need a separate test with an appropriate account; mocked HTTP tests do not validate provider quota or real model output.

Keep API keys, recordings, meeting transcripts, databases, model weights, native runtimes and build output out of commits. Use synthetic fixtures for tests. Track `pnpm-lock.yaml` and `src-tauri/Cargo.lock`.

For bugs, include reproduction steps, Windows version, audio devices, selected providers and sanitized logs. Report vulnerabilities using [SECURITY.md](SECURITY.md).

Contributions are licensed under the [MIT License](LICENSE). Preserve upstream copyright and attribution.
