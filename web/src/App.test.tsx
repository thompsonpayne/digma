import { render, screen } from "@solidjs/testing-library";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import App from "./App";
import { ToolMode } from "./editorTypes";

const syncTool = vi.fn();

vi.mock("./createCanvasInputController", () => ({
  createCanvasInputController: () => ({
    syncTool,
    ensureBatch: vi.fn(),
    getBatch: vi.fn(() => null),
    clearBatchEvents: vi.fn(),
    isPanning: vi.fn(() => false),
    isSpaceDown: vi.fn(() => false),
  }),
}));

vi.mock("./createWasmApp", () => ({
  createWasmApp: () => ({
    camera: () => ({ pan: { x: 10, y: 20 }, zoom: 1.5 }),
    error: () => null,
    version: () => "test-version",
  }),
}));

describe("App", () => {
  beforeEach(() => {
    syncTool.mockClear();
  });

  it("shows the wasm version", () => {
    render(() => <App />);
    expect(screen.getByText(/wasm version: test-version/i)).toBeInTheDocument();
  });

  it("switches to rectangle mode when clicked", async () => {
    const user = userEvent.setup();
    render(() => <App />);

    await user.click(screen.getByRole("button", { name: /rectangle/i }));

    expect(syncTool).toHaveBeenCalledWith(ToolMode.rect);
  });
});
