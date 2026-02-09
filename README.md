# Dikto

Voice-to-text for macOS. Records speech and transcribes it locally — no cloud APIs.

> macOS 14+ (Sonoma) &middot; Apple Silicon

[![CI](https://github.com/sourabhbgp/dikto/actions/workflows/ci.yml/badge.svg)](https://github.com/sourabhbgp/dikto/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

## Install

### Homebrew (recommended)

```bash
brew tap sourabhbgp/dikto
```

**GUI app:**

```bash
brew install --cask dikto
```

**CLI only** (builds from source):

```bash
brew install sourabhbgp/dikto/dikto
```

### GitHub Releases

Download the latest DMG from [Releases](https://github.com/sourabhbgp/dikto/releases).

Dikto is ad-hoc signed (not notarized). After installing, right-click the app and choose **Open**, or run:

```bash
xattr -cr /Applications/Dikto.app
```

### Build from source

Prerequisites: [Rust](https://rustup.rs/) (1.75+), cmake (`brew install cmake`), macOS 14+.

```bash
git clone https://github.com/sourabhbgp/dikto.git
cd dikto
make build-app
open build/Dikto.app
```

## Usage

Press **Option+R** to start recording. Speech is transcribed when you stop or silence is detected. The text is copied to your clipboard and pasted into the active app.

On first launch, macOS will prompt for **Microphone** access. Grant **Accessibility** permission in System Settings for auto-paste.

## Models

| Model | Size | Description |
|---|---|---|
| `parakeet-tdt-0.6b-v2` (default) | 2.5 GB | NVIDIA Parakeet TDT — high accuracy English |
| `parakeet-tdt-0.6b-v3` | 2.6 GB | NVIDIA Parakeet TDT — 25 EU languages |
| `whisper-tiny` | 75 MB | Whisper Tiny — fast, 99 languages |
| `whisper-small` | 460 MB | Whisper Small — balanced accuracy & speed |
| `whisper-large-v3-turbo` | 1.6 GB | Whisper Large v3 Turbo — highest accuracy |
| `distil-whisper-large-v3` | 1.5 GB | Distil-Whisper — 6x faster Whisper |

Download a model with the CLI:

```bash
dikto --setup --model whisper-small
```

Then select it in the app's Settings.

## Architecture

- **Rust core** (`dikto-core`) — audio capture, VAD, ASR engine, model management
- **Swift UI** (`DiktoApp`) — SwiftUI menu-bar app with recording overlay
- **CLI** (`dikto-cli`) — headless model setup

Config: `~/.config/dikto/config.json` &middot; Models: `~/.local/share/dikto/models/`

## License

[MIT](LICENSE)
