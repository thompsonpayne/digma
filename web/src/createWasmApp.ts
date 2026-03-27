import type { Accessor } from "solid-js";
import { createSignal, onCleanup, onMount } from "solid-js";
import type { createCanvasInputController } from "./createCanvasInputController";
import type { CameraView, TickOutput, ToolModeType } from "./editorTypes";

type WasmAppInstance = import("./wasm/app_wasm/app_wasm").App;
type WasmModule = typeof import("./wasm/app_wasm/app_wasm");

type CreateWasmAppOptions = {
  canvas: Accessor<HTMLCanvasElement>;
  input: ReturnType<typeof createCanvasInputController>;
  toolMode: Accessor<ToolModeType>;
};

const CURSOR_MAP: Record<string, string> = {
  default: "default",
  resize_tl_br: "nwse-resize",
  resize_tr_bl: "nesw-resize",
  move: "move",
  crosshair: "crosshair",
  pan: "grab",
  panning: "grabbing",
};

function resolveCursor(
  cursor: string,
  isPanning: boolean,
  isSpaceDown: boolean,
): string {
  if (isPanning) {
    return "grabbing";
  }

  if (isSpaceDown) {
    return "grab";
  }

  return CURSOR_MAP[cursor] ?? "default";
}

export function createWasmApp(options: CreateWasmAppOptions) {
  const [camera, setCamera] = createSignal<CameraView | null>(null);
  const [error, setError] = createSignal<string | null>(null);
  const [version, setVersion] = createSignal("(loading)");

  onMount(() => {
    const canvas = options.canvas();
    let app: WasmAppInstance | null = null;
    let rafId = 0;
    let running = true;

    const startFrameLoop = (): void => {
      if (!app) {
        return;
      }

      const frame = () => {
        if (!running || !app) {
          return;
        }

        try {
          const output = app.tick(options.input.getBatch()) as TickOutput;
          options.input.clearBatchEvents();
          setCamera(output.camera);
          setError(null);
          canvas.style.cursor = resolveCursor(
            output.cursor,
            options.input.isPanning(),
            options.input.isSpaceDown(),
          );
        } catch (err) {
          setError(`tick error: ${String(err)}`);
        }

        rafId = requestAnimationFrame(frame);
      };

      rafId = requestAnimationFrame(frame);
    };

    void (async () => {
      try {
        const wasm: WasmModule = await import("./wasm/app_wasm/app_wasm");
        if (!running) {
          return;
        }

        await wasm.default();
        if (!running) {
          return;
        }

        setVersion(wasm.version());

        const nextApp = await wasm.App.new(canvas);
        if (!running) {
          nextApp.free();
          return;
        }

        app = nextApp;
        options.input.ensureBatch(options.toolMode());
        startFrameLoop();
      } catch (err) {
        setError(`init error: ${String(err)}`);
      }
    })();

    onCleanup(() => {
      running = false;
      if (rafId) {
        cancelAnimationFrame(rafId);
      }
      app?.free();
    });
  });

  return {
    camera,
    error,
    version,
  };
}
