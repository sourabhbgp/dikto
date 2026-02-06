import { spawn } from "node:child_process";
import { mkdtemp, rm } from "node:fs/promises";
import { join } from "node:path";
import { tmpdir } from "node:os";
import type { RecordingOptions, RecordingResult } from "./types.js";

export function checkSox(): Promise<void> {
  return new Promise((resolve, reject) => {
    const proc = spawn("which", ["rec"]);
    proc.on("close", (code) => {
      if (code === 0) {
        resolve();
      } else {
        reject(
          new Error(
            "sox is not installed. Install it with: brew install sox"
          )
        );
      }
    });
    proc.on("error", () => {
      reject(
        new Error(
          "sox is not installed. Install it with: brew install sox"
        )
      );
    });
  });
}

export async function record(options: RecordingOptions): Promise<RecordingResult> {
  await checkSox();

  const tempDir = await mkdtemp(join(tmpdir(), "sotto-"));
  const filePath = join(tempDir, "recording.wav");

  const startTime = Date.now();

  return new Promise<RecordingResult>((resolve, reject) => {
    // rec -c 1 -r 16000 -b 16 -e signed-integer output.wav
    //   trim 0 <maxDuration>
    //   silence 1 0.1 1%      ← skip leading silence
    //   1 <silenceDuration> <silenceThreshold>  ← stop after trailing silence
    const args = [
      "-c", "1",              // mono
      "-r", "16000",          // 16kHz sample rate
      "-b", "16",             // 16-bit
      "-e", "signed-integer", // signed integer encoding
      filePath,
      "trim", "0", String(options.maxDuration),
      "silence",
      "1", "0.1", "1%",      // skip leading silence (wait for speech)
      "1", String(options.silenceDuration), options.silenceThreshold, // stop on trailing silence
    ];

    const proc = spawn("rec", args, {
      stdio: ["ignore", "ignore", "ignore"],
    });

    const timeout = setTimeout(() => {
      proc.kill("SIGTERM");
    }, (options.maxDuration + 2) * 1000);

    proc.on("close", (code) => {
      clearTimeout(timeout);
      const durationMs = Date.now() - startTime;

      if (code === 0 || code === null) {
        resolve({ filePath, durationMs });
      } else {
        reject(new Error(`Recording failed with exit code ${code}`));
      }
    });

    proc.on("error", (err) => {
      clearTimeout(timeout);
      reject(
        new Error(`Failed to start recording: ${err.message}`)
      );
    });
  });
}

export async function cleanupRecording(filePath: string): Promise<void> {
  try {
    // Remove the temp directory containing the recording
    const dir = join(filePath, "..");
    await rm(dir, { recursive: true, force: true });
  } catch {
    // Ignore cleanup errors
  }
}
