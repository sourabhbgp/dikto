import { describe, it, expect } from "vitest";
import { createServer } from "../src/server.js";

describe("server", () => {
  it("should create an MCP server instance", () => {
    const server = createServer();
    expect(server).toBeDefined();
  });
});
