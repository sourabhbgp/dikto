import { describe, it, expect, vi, beforeEach } from "vitest";
import type { ChildProcess } from "node:child_process";

// Mock child_process
const mockStdinWrite = vi.fn().mockReturnValue(true);
const mockKill = vi.fn();
let mockProc: Partial<ChildProcess> & { exitCode: number | null };

vi.mock("node:child_process", () => {
  const EventEmitter = require("node:events");

  return {
    spawn: vi.fn(() => {
      const proc = new EventEmitter();
      proc.stdin = { write: mockStdinWrite };
      proc.stdout = null;
      proc.stderr = null;
      proc.kill = mockKill;
      proc.exitCode = null;
      mockProc = proc;
      return proc;
    }),
  };
});

// Import after mocking
const { StatusIndicator } = await import("../src/indicator.js");
const { spawn } = await import("node:child_process");

describe("StatusIndicator", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  describe("show", () => {
    it("should spawn osascript with JavaScript flag", () => {
      const indicator = new StatusIndicator();
      indicator.show("listening");

      expect(spawn).toHaveBeenCalledWith(
        "osascript",
        ["-l", "JavaScript", "-e", expect.any(String)],
        { stdio: ["pipe", "ignore", "ignore"] }
      );
    });

    it("should write the initial status to stdin", () => {
      const indicator = new StatusIndicator();
      indicator.show("listening");

      expect(mockStdinWrite).toHaveBeenCalledWith("listening\n");
    });

    it("should write transcribing status when shown with that state", () => {
      const indicator = new StatusIndicator();
      indicator.show("transcribing");

      expect(mockStdinWrite).toHaveBeenCalledWith("transcribing\n");
    });
  });

  describe("update", () => {
    it("should write new status to stdin", () => {
      const indicator = new StatusIndicator();
      indicator.show("listening");
      mockStdinWrite.mockClear();

      indicator.update("transcribing");

      expect(mockStdinWrite).toHaveBeenCalledWith("transcribing\n");
    });

    it("should no-op when not showing", () => {
      const indicator = new StatusIndicator();
      indicator.update("transcribing");

      expect(mockStdinWrite).not.toHaveBeenCalled();
    });
  });

  describe("sendText", () => {
    it("should write text: command to stdin", () => {
      const indicator = new StatusIndicator();
      indicator.show("listening");
      mockStdinWrite.mockClear();

      indicator.sendText("Hello world");

      expect(mockStdinWrite).toHaveBeenCalledWith("text:Hello world\n");
    });

    it("should replace newlines with spaces", () => {
      const indicator = new StatusIndicator();
      indicator.show("listening");
      mockStdinWrite.mockClear();

      indicator.sendText("line one\nline two\rline three");

      expect(mockStdinWrite).toHaveBeenCalledWith("text:line one line two line three\n");
    });

    it("should no-op when not showing", () => {
      const indicator = new StatusIndicator();
      indicator.sendText("Hello");

      expect(mockStdinWrite).not.toHaveBeenCalled();
    });

    it("should not throw if stdin.write throws", () => {
      const indicator = new StatusIndicator();
      indicator.show("listening");
      mockStdinWrite.mockImplementationOnce(() => {
        throw new Error("write after end");
      });

      expect(() => indicator.sendText("Hello")).not.toThrow();
    });
  });

  describe("close", () => {
    it("should write close command to stdin", () => {
      const indicator = new StatusIndicator();
      indicator.show("listening");
      mockStdinWrite.mockClear();

      indicator.close();

      expect(mockStdinWrite).toHaveBeenCalledWith("close\n");
    });

    it("should no-op when not showing", () => {
      const indicator = new StatusIndicator();
      indicator.close();

      expect(mockStdinWrite).not.toHaveBeenCalled();
    });

    it("should force-kill after timeout if process still running", () => {
      vi.useFakeTimers();

      const indicator = new StatusIndicator();
      indicator.show("listening");
      indicator.close();

      expect(mockKill).not.toHaveBeenCalled();

      vi.advanceTimersByTime(500);

      expect(mockKill).toHaveBeenCalled();

      vi.useRealTimers();
    });

    it("should not force-kill if process already exited", () => {
      vi.useFakeTimers();

      const indicator = new StatusIndicator();
      indicator.show("listening");
      mockProc.exitCode = 0;
      indicator.close();

      vi.advanceTimersByTime(500);

      expect(mockKill).not.toHaveBeenCalled();

      vi.useRealTimers();
    });
  });

  describe("graceful degradation", () => {
    it("should not throw if spawn throws", () => {
      vi.mocked(spawn).mockImplementationOnce(() => {
        throw new Error("osascript not found");
      });

      const indicator = new StatusIndicator();
      expect(() => indicator.show("listening")).not.toThrow();
    });

    it("should not throw if stdin.write throws on update", () => {
      const indicator = new StatusIndicator();
      indicator.show("listening");
      mockStdinWrite.mockImplementationOnce(() => {
        throw new Error("write after end");
      });

      expect(() => indicator.update("transcribing")).not.toThrow();
    });

    it("should not throw if stdin.write throws on close", () => {
      const indicator = new StatusIndicator();
      indicator.show("listening");
      mockStdinWrite.mockImplementationOnce(() => {
        throw new Error("write after end");
      });

      expect(() => indicator.close()).not.toThrow();
    });
  });
});
