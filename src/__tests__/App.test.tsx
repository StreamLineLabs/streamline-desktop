import { describe, it, expect, vi } from "vitest";

// Mock Tauri invoke API before importing anything that uses it
vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
  isTauri: vi.fn(() => false),
}));

describe("StreamlineDesktop", () => {
  it("should define app module", async () => {
    const App = await import("../App");
    expect(App).toBeDefined();
    expect(App.default).toBeDefined();
  });

  it("should have valid type definitions", async () => {
    const types = await import("../types");
    expect(types).toBeDefined();
  });

  it("should export COLORS constant", async () => {
    const { COLORS } = await import("../types");
    expect(COLORS).toBeDefined();
    expect(COLORS.bg).toBe("#0f0f23");
    expect(COLORS.green).toBe("#4caf50");
    expect(COLORS.red).toBe("#f44336");
  });

  it("should export style constants", async () => {
    const { inputStyle, btnStyle, thStyle, tdStyle } = await import("../types");
    expect(inputStyle).toBeDefined();
    expect(inputStyle.width).toBe("100%");
    expect(btnStyle).toBeDefined();
    expect(btnStyle.cursor).toBe("pointer");
    expect(thStyle).toBeDefined();
    expect(tdStyle).toBeDefined();
  });
});
