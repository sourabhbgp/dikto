import { describe, it, expect, vi } from "vitest";
import { checkSox } from "../src/recorder.js";

// Mock child_process for unit tests since we don't want to actually run sox
vi.mock("node:child_process", () => {
  const EventEmitter = require("node:events");

  return {
    spawn: vi.fn((cmd: string, args: string[]) => {
      const proc = new EventEmitter();
      proc.stdout = new EventEmitter();
      proc.stderr = new EventEmitter();
      proc.stdin = null;
      proc.kill = vi.fn();

      if (cmd === "which" && args[0] === "rec") {
        // Simulate "which rec" finding the command
        setTimeout(() => proc.emit("close", 0), 0);
      } else if (cmd === "rec") {
        // Simulate a short recording
        setTimeout(() => proc.emit("close", 0), 10);
      }

      return proc;
    }),
  };
});

describe("recorder", () => {
  describe("checkSox", () => {
    it("should resolve when sox/rec is available", async () => {
      await expect(checkSox()).resolves.toBeUndefined();
    });
  });
});
