import { describe, it, expect } from "vitest";
import { parseWhisperOutput } from "../src/transcriber.js";

describe("transcriber", () => {
  describe("parseWhisperOutput", () => {
    it("should parse clean text output", () => {
      const result = parseWhisperOutput("Hello, world!\n");
      expect(result).toBe("Hello, world!");
    });

    it("should handle multiple lines", () => {
      const result = parseWhisperOutput("Hello world.\nHow are you?\n");
      expect(result).toBe("Hello world. How are you?");
    });

    it("should filter out whisper system lines", () => {
      const raw = [
        "whisper_init_from_file: loading model...",
        "whisper_model_load: loading model",
        "Hello, this is a test.",
        "main: some log line",
      ].join("\n");
      const result = parseWhisperOutput(raw);
      expect(result).toBe("Hello, this is a test.");
    });

    it("should return no speech detected for empty output", () => {
      const result = parseWhisperOutput("");
      expect(result).toBe("[No speech detected]");
    });

    it("should return no speech detected for whitespace-only output", () => {
      const result = parseWhisperOutput("   \n  \n  ");
      expect(result).toBe("[No speech detected]");
    });

    it("should trim leading and trailing whitespace from lines", () => {
      const result = parseWhisperOutput("  Hello world.  \n  Goodbye.  \n");
      expect(result).toBe("Hello world. Goodbye.");
    });

    it("should filter lines starting with whisper_ prefix", () => {
      const raw = "whisper_full_default_params: something\nActual text here\n";
      const result = parseWhisperOutput(raw);
      expect(result).toBe("Actual text here");
    });
  });
});
