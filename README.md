# sotto

*Sotto voce* — in a soft voice.

Voice input for Claude Code. Speak instead of typing.

A local, open-source MCP server that records your voice, transcribes it using [whisper.cpp](https://github.com/ggerganov/whisper.cpp), and sends the text to Claude Code. Everything runs on your machine — no cloud APIs, no network calls for transcription.

## How It Works

```
You type /listen → Claude calls the "listen" tool → MCP server records audio via sox
→ silence detected, recording stops → whisper.cpp transcribes locally → text returned
→ Claude treats it as your message and responds
```

## Prerequisites

- **macOS** with Apple Silicon (Intel works too, just slower)
- **Node.js** >= 18
- **sox** — audio recording
- **whisper-cpp** — local speech-to-text

Install system dependencies:

```bash
brew install sox whisper-cpp
```

## Installation

```bash
npm install -g sotto
sotto-setup
```

The setup command will:
1. Verify sox and whisper-cpp are installed
2. Download the Whisper Base English model (~150MB) to `~/.local/share/sotto/models/`
3. Create a default config at `~/.config/sotto/config.json`

Then register with Claude Code:

```bash
claude mcp add sotto -- sotto
```

## Usage

In Claude Code, type:

```
/mcp__sotto__listen
```

Then speak into your microphone. The recording automatically stops after 2 seconds of silence. Your speech is transcribed and sent to Claude as text.

## Configuration

Edit `~/.config/sotto/config.json`:

| Setting | Default | Env Var | Description |
|---|---|---|---|
| `modelPath` | `~/.local/share/sotto/models/ggml-base.en.bin` | `WHISPER_MODEL_PATH` | Path to GGML model |
| `language` | `en` | `WHISPER_LANGUAGE` | Language code |
| `maxDuration` | `30` | `WHISPER_MAX_DURATION` | Max recording seconds |
| `silenceDuration` | `2` | — | Seconds of silence before auto-stop |
| `silenceThreshold` | `3%` | — | Silence detection threshold |

Environment variables take precedence over the config file.

## Troubleshooting

| Problem | Solution |
|---|---|
| "sox is not installed" | `brew install sox` |
| "whisper-cpp is not installed" | `brew install whisper-cpp` |
| "Model not found" | Run `sotto-setup` |
| "Microphone access denied" | Grant mic access to your terminal in System Settings > Privacy & Security > Microphone |
| No speech detected | Make sure your microphone is working and you're speaking loudly enough |
| Transcription is slow | The base model is ~3s for a 5s clip on Apple Silicon. Try the tiny model for faster results. |

## Development

```bash
git clone <repo-url>
cd sotto
npm install
npm run build
npm test
```

## License

MIT
