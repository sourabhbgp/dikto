import { spawn } from "node:child_process";
import { access } from "node:fs/promises";
import type { TranscriptionOptions, TranscriptionResult } from "./types.js";

export function checkWhisperCpp(): Promise<void> {
  return new Promise((resolve, reject) => {
    const proc = spawn("which", ["whisper-cli"]);
    proc.on("close", (code) => {
      if (code === 0) {
        resolve();
      } else {
        reject(
          new Error(
            "whisper-cpp is not installed. Install it with: brew install whisper-cpp"
          )
        );
      }
    });
    proc.on("error", () => {
      reject(
        new Error(
          "whisper-cpp is not installed. Install it with: brew install whisper-cpp"
        )
      );
    });
  });
}

export async function checkModel(modelPath: string): Promise<void> {
  try {
    await access(modelPath);
  } catch {
    throw new Error(
      `Whisper model not found at ${modelPath}. Run: whisper-mcp-setup`
    );
  }
}

export async function transcribe(
  audioPath: string,
  options: TranscriptionOptions
): Promise<TranscriptionResult> {
  await checkWhisperCpp();
  await checkModel(options.modelPath);

  return new Promise<TranscriptionResult>((resolve, reject) => {
    const args = [
      "-m", options.modelPath,
      "-f", audioPath,
      "-l", options.language,
      "-nt",  // no timestamps
      "-np",  // no progress
    ];

    const proc = spawn("whisper-cli", args, {
      stdio: ["ignore", "pipe", "pipe"],
    });

    let stdout = "";
    let stderr = "";

    proc.stdout.on("data", (data: Buffer) => {
      stdout += data.toString();
    });

    proc.stderr.on("data", (data: Buffer) => {
      stderr += data.toString();
    });

    proc.on("close", (code) => {
      if (code !== 0) {
        reject(
          new Error(
            `whisper-cpp failed (exit code ${code}): ${stderr.trim()}`
          )
        );
        return;
      }

      const text = parseWhisperOutput(stdout);
      resolve({ text });
    });

    proc.on("error", (err) => {
      reject(new Error(`Failed to run whisper-cpp: ${err.message}`));
    });
  });
}

export function parseWhisperOutput(raw: string): string {
  // whisper-cpp outputs text with possible leading/trailing whitespace
  // and sometimes blank lines. Clean it up.
  const text = raw
    .split("\n")
    .map((line) => line.trim())
    .filter((line) => line.length > 0)
    // Filter out whisper-cpp system/log lines
    .filter((line) => !line.startsWith("whisper_"))
    .filter((line) => !line.startsWith("main:"))
    .join(" ")
    .trim();

  return text || "[No speech detected]";
}
