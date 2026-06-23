<div align="center">

# Zortbit 🤖

### Your own AI for your own mess.

**It learns how *you* file things — and does it 100% on your Mac.**

![macOS 13+](https://img.shields.io/badge/macOS-13%2B-111?logo=apple&logoColor=white)
&nbsp;![Rust + Tauri](https://img.shields.io/badge/Rust%20%2B%20Tauri-ce422b?logo=rust&logoColor=white)
&nbsp;![100% on-device](https://img.shields.io/badge/100%25-on--device-3a8ee6)
&nbsp;![License: MIT](https://img.shields.io/badge/license-MIT-3fae5a)

<img src="docs/demo.png" alt="Zortbit reading files and proposing tidy names and the right folder" width="840">

</div>

Zortbit is a tiny macOS menu-bar app that watches your Downloads, reads what files actually
*are* (OCR for screenshots, text for documents), and proposes a tidy name plus the right
folder — using a **local** model. Nothing leaves your machine, nothing is ever permanently
deleted, and every move is reversible.

## Why

Folders fill up with `Scan_2026_03_18 (1).pdf` and `Screenshot 2026-… .png`. Rule-based
organizers can't tell what's *inside* a file, and cloud "AI organizers" send your
documents off your device. Zortbit does it locally and gets better as it learns which
folders you actually approve.

## Features

- 🧠 **Content-aware** — Apple Vision OCR reads screenshots/images; built-in macOS tools
  read PDF / DOCX / PPTX / RTF — so files sort by what they contain, not just their name.
- 🗂️ **Project-aware** — a small local model files into *your* folders, and learns from
  what you approve over time.
- 🔒 **Local-first & private** — runs entirely on your Mac. Files never leave the device.
- ↩️ **Safe by design** — propose-first (nothing moves until you approve); **never**
  permanently deletes (Trash only, with one-click Undo); sensitive files (keys,
  credentials, `.env`) are quarantined locally and never sent to a model.
- 🪶 **Lightweight** — a menu-bar popover that sleeps when idle; the model loads on demand.

## Requirements

- macOS 13+ (Apple Silicon)
- [Rust](https://rustup.rs) and [Node.js](https://nodejs.org)
- [Ollama](https://ollama.com) with a small model: `ollama pull qwen2.5:3b`
- Xcode Command Line Tools (`xcode-select --install`) — builds the OCR sidecar.
  Optional: without it, Zortbit still files by name and type.

## Run (dev)

```sh
ollama serve &          # start the local model server
npm install
npm run tauri dev
```

Zortbit appears in your menu bar — click the icon, then **Scan my mess**.

## Build a .app

```sh
npm run tauri build -- --bundles app
# → src-tauri/target/release/bundle/macos/Zortbit.app
```

## How it works

1. A file watcher (FSEvents) wakes on new files in your configured folders.
2. Content is extracted **locally** — Vision OCR for images, built-in tools for documents.
3. A local model (`qwen2.5:3b` via Ollama) picks one folder from a closed list, and the
   file is renamed kebab-case. Unclassifiable files fall back to a type folder.
4. You approve; the move is logged so it stays reversible and so Zortbit learns your style.

## Configuration

Edit `~/Library/Application Support/com.xaviour.zortbit/config.json`:

| Key | Meaning |
|---|---|
| `categories` | Your project/area folders — the closed list the model chooses from |
| `bulk_scope` | Folders to scan |
| `protected` | Folders Zortbit must never touch |
| `organize_base` | Where organized files go (default `~/Organized`, kept local) |
| `model` | The model id (e.g. `qwen2.5:3b`) |
| `provider` | `ollama` (default) or `openai` for any OpenAI-compatible server |
| `endpoint` | OpenAI-compatible chat URL (used when `provider` is `openai`) |
| `automation` | `propose` (default) · or `auto` to file trusted patterns in the background |

### Use any local model — Ollama, Foundry Local, LM Studio…

Zortbit talks to a local model server. By default that's [Ollama](https://ollama.com). To use
any other **OpenAI-compatible** local server — including **Microsoft Foundry Local**, LM Studio,
or `llama.cpp`'s server — set this in `config.json`:

```json
{ "provider": "openai", "endpoint": "http://localhost:PORT/v1/chat/completions", "model": "your-model" }
```

For Foundry Local, start the service and read its endpoint from `foundry service status`, then
drop that URL in. Everything still runs on your machine — nothing goes to the cloud.

### Automatic mode (it learns)

By default Zortbit is **propose-first** — nothing moves until you approve. Once it has seen you
approve the same kind of move a few times, set `"automation": "auto"` and it will **file those
trusted, high-confidence moves silently in the background** as new files arrive. It will **never**
auto-delete and **never** auto-touch sensitive files — those always wait for your explicit ok.

## Roadmap — what's coming

- **Gentle Mode scheduler** (the "Gentle · 3%" in the screenshot) — tidy quietly in the
  background, throttled by system load, with an ETA and a heads-up before shutdown.
- **One-click model presets** in onboarding — pick Ollama, Microsoft Foundry Local, or LM Studio
  without hand-editing config.
- **Pin-your-own rules** — "always send invoices to Finance", "never touch `~/Code`".
- **Signed & notarized `.app`** + auto-update, so it shares without Gatekeeper warnings.
- **Optional local vision model** for photos that have no text.
- **Windows & Linux** builds.
- A weekly **tidy digest** — what got filed, and what's still waiting on you.

Ideas and PRs for any of these are very welcome.

## Contributing

Issues and PRs welcome. Built with Rust and [Tauri](https://tauri.app).

## License

[MIT](LICENSE) © 2026 Christopher Herrera Magana
