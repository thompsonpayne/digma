import { createMemo, createSignal, Show } from "solid-js";

import { createCanvasInputController } from "./createCanvasInputController";
import { createWasmApp } from "./createWasmApp";
import type { InputEvent, ToolMode as ToolModeValue } from "./editorTypes";
import { ToolMode } from "./editorTypes";
import { hexToRgbaColor } from "./utils";

function App() {
  let canvasRef!: HTMLCanvasElement;

  const [toolMode, setToolMode] = createSignal<ToolModeValue>(ToolMode.select);
  const [fillColor, setFillColor] = createSignal<string>("");

  const input = createCanvasInputController({
    canvas: () => canvasRef,
    onToolChange: (nextTool) => {
      setToolMode(nextTool);
      input.syncTool(nextTool);
    },
    toolMode,
  });

  const wasm = createWasmApp({
    canvas: () => canvasRef,
    input,
    toolMode,
  });

  const cameraText = createMemo(() => {
    const error = wasm.error();
    if (error) {
      return error;
    }

    const camera = wasm.camera();
    if (!camera) {
      return "(no data)";
    }

    return `pan=(${camera.pan.x.toFixed(2)}, ${camera.pan.y.toFixed(2)}), zoom=${camera.zoom.toFixed(3)}`;
  });

  const selectTool = (nextTool: ToolModeValue): void => {
    setToolMode(nextTool);
    input.syncTool(nextTool);
  };

  const handleFillInput = (hex: string): void => {
    setFillColor(hex);

    const color = hexToRgbaColor(hex);
    if (!color) {
      return;
    }

    input.pushEvent({
      type: "set_selection_fill",
      color,
    } as InputEvent);
  };

  return (
    <div class="min-h-screen bg-stone-100 px-4 py-6 text-stone-900 sm:px-6">
      <div class="mx-auto flex max-w-6xl flex-col gap-4">
        <div class="flex flex-wrap items-center justify-between gap-3 rounded-2xl border border-stone-300 bg-white/90 px-4 py-3 shadow-sm backdrop-blur">
          <div>
            <p class="text-sm font-medium uppercase tracking-[0.2em] text-stone-500">
              Digma
            </p>
            <p class="text-sm text-stone-600">wasm version: {wasm.version()}</p>
          </div>
          <p class="text-sm text-stone-600">{cameraText()}</p>
        </div>

        <div
          id="toolbar"
          class="flex flex-wrap items-center gap-3 rounded-2xl border border-stone-300 bg-white px-4 py-3 shadow-sm"
        >
          <span class="text-sm font-medium text-stone-600">Tool mode</span>
          <button
            type="button"
            onClick={() => selectTool(ToolMode.select)}
            class={`rounded-full border px-4 py-2 text-sm font-medium transition ${
              toolMode() === ToolMode.select
                ? "border-stone-900 bg-stone-900 text-white"
                : "border-stone-300 bg-stone-50 text-stone-700 hover:border-stone-400 hover:bg-stone-100"
            }`}
          >
            Select
          </button>
          <button
            type="button"
            onClick={() => selectTool(ToolMode.rect)}
            class={`rounded-full border px-4 py-2 text-sm font-medium transition ${
              toolMode() === ToolMode.rect
                ? "border-amber-600 bg-amber-500 text-white"
                : "border-stone-300 bg-stone-50 text-stone-700 hover:border-stone-400 hover:bg-stone-100"
            }`}
          >
            Rectangle
          </button>
          <span class="text-sm text-stone-500">Hold space to pan</span>

          <div class="flex items-center">
            <label for="color-fill-picker">Fill color</label>
            <input
              name="color-fill-picker"
              type="color"
              value={fillColor()}
              onChange={(e) => {
                if (
                  e.currentTarget.value !== null &&
                  e.currentTarget.value !== undefined
                ) {
                  handleFillInput(e.currentTarget.value);
                }
              }}
            ></input>
          </div>

          <Show when={fillColor()}>
            <div>
              <span>Fill color is: </span>
              <span class="font-bold" style={{ color: `${fillColor()}` }}>
                {fillColor()}
              </span>
            </div>
          </Show>
        </div>

        <div class="overflow-hidden rounded-3xl border border-stone-300 bg-white p-3 shadow-lg shadow-stone-300/40">
          <canvas
            ref={canvasRef}
            class="block h-auto max-w-full rounded-2xl border border-stone-200 bg-stone-50"
            width={800}
            height={600}
          ></canvas>
        </div>
      </div>
    </div>
  );
}

export default App;
