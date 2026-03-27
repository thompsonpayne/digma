import type { Accessor } from "solid-js";
import { onCleanup, onMount } from "solid-js";
import type {
  InputBatch,
  InputEvent,
  Point,
  ToolModeType as ToolModeValue,
} from "./editorTypes";
import { ToolMode } from "./editorTypes";

type InteractionState =
  | { kind: "idle" }
  | { kind: "panning"; pointerId: number; lastPt: Point }
  | { kind: "selecting"; pointerId: number }
  | { kind: "rectCreating"; pointerId: number };

type CreateCanvasInputControllerOptions = {
  canvas: Accessor<HTMLCanvasElement>;
  toolMode: Accessor<ToolModeValue>;
  onToolChange: (nextTool: ToolModeValue) => void;
};

const IDLE_INTERACTION: InteractionState = { kind: "idle" };

function toCanvasPoint(
  canvas: HTMLCanvasElement,
  event: PointerEvent | WheelEvent,
): Point {
  const rect = canvas.getBoundingClientRect();

  return {
    x: event.clientX - rect.left,
    y: event.clientY - rect.top,
  };
}

function releasePointerCapture(
  canvas: HTMLCanvasElement,
  pointerId: number,
): void {
  if (canvas.hasPointerCapture(pointerId)) {
    canvas.releasePointerCapture(pointerId);
  }
}

export function createCanvasInputController(
  options: CreateCanvasInputControllerOptions,
) {
  let batch: InputBatch | null = null;
  let interaction: InteractionState = IDLE_INTERACTION;
  let spaceDown = false;
  let pendingToolReset: ToolModeValue | null = null;

  const getBatch = (): InputBatch | null => batch;

  const ensureBatch = (tool: ToolModeValue): InputBatch => {
    if (!batch) {
      batch = { events: [], tool };
    }

    return batch;
  };

  const pushEvent = (event: InputEvent): void => {
    ensureBatch(options.toolMode()).events.push(event);
  };

  const clearBatchEvents = (): void => {
    if (batch) {
      batch.events.length = 0;
    }

    if (pendingToolReset) {
      const nextTool = pendingToolReset;
      pendingToolReset = null;
      options.onToolChange(nextTool);
    }
  };

  const syncTool = (nextTool: ToolModeValue): void => {
    if (batch) {
      batch.tool = nextTool;
    }
  };

  const isPanning = (): boolean => interaction.kind === "panning";
  const isSpaceDown = (): boolean => spaceDown;

  onMount(() => {
    const canvas = options.canvas();
    const abortController = new AbortController();

    window.addEventListener(
      "keydown",
      (event) => {
        if (event.code === "Space") {
          spaceDown = true;
          event.preventDefault();
        }

        const isPrimaryModifer = event.metaKey || event.ctrlKey;
        const key = event.key.toLowerCase();

        if (isPrimaryModifer && key === "z") {
          pushEvent({ type: event.shiftKey ? "redo" : "undo" });
          event.preventDefault();
          return;
        }

        if (isPrimaryModifer && key === "y") {
          pushEvent({ type: "redo" });
          event.preventDefault();
          return;
        }

        if (key === "d") {
          pushEvent({ type: "delete_selected" });
          event.preventDefault();
          return;
        }
      },
      { signal: abortController.signal },
    );

    window.addEventListener(
      "keyup",
      (event) => {
        if (event.code !== "Space") {
          return;
        }

        if (interaction.kind === "panning") {
          releasePointerCapture(canvas, interaction.pointerId);
        }

        spaceDown = false;
        interaction = IDLE_INTERACTION;
        event.preventDefault();
      },
      { signal: abortController.signal },
    );

    canvas.addEventListener(
      "pointerdown",
      (event) => {
        if (event.button === 0 && spaceDown) {
          interaction = {
            kind: "panning",
            pointerId: event.pointerId,
            lastPt: { x: event.clientX, y: event.clientY },
          };
          canvas.setPointerCapture(event.pointerId);
          canvas.style.cursor = "grabbing";
          event.preventDefault();
          return;
        }

        if (event.button !== 0 || !batch) {
          return;
        }

        interaction =
          options.toolMode() === ToolMode.select
            ? { kind: "selecting", pointerId: event.pointerId }
            : { kind: "rectCreating", pointerId: event.pointerId };

        canvas.setPointerCapture(event.pointerId);
        batch.events.push({
          type: "pointer_down",
          screen_px: toCanvasPoint(canvas, event),
          shift: event.shiftKey,
          button: event.button,
        });
      },
      { signal: abortController.signal },
    );

    canvas.addEventListener(
      "pointermove",
      (event) => {
        if (interaction.kind === "panning" && batch) {
          const dx = event.clientX - interaction.lastPt.x;
          const dy = event.clientY - interaction.lastPt.y;

          batch.events.push({
            type: "camera_pan_by_screen_delta",
            delta_px: { x: dx, y: dy },
          });
          interaction = {
            ...interaction,
            lastPt: { x: event.clientX, y: event.clientY },
          };
          canvas.style.cursor = "grabbing";
          return;
        }

        if (batch) {
          batch.events.push({
            type: "pointer_move",
            screen_px: toCanvasPoint(canvas, event),
            buttons: event.buttons,
          });
        }
      },
      { signal: abortController.signal },
    );

    canvas.addEventListener(
      "pointerup",
      (event) => {
        if (interaction.kind === "panning") {
          releasePointerCapture(canvas, interaction.pointerId);
          interaction = IDLE_INTERACTION;
          return;
        }

        if (
          (interaction.kind === "selecting" ||
            interaction.kind === "rectCreating") &&
          batch
        ) {
          const finishedInteraction = interaction;
          interaction = IDLE_INTERACTION;

          if (finishedInteraction.kind === "rectCreating") {
            pendingToolReset = ToolMode.select;
            // options.onToolChange(ToolMode.select);
          }

          releasePointerCapture(canvas, finishedInteraction.pointerId);
          batch.events.push({
            type: "pointer_up",
            screen_px: toCanvasPoint(canvas, event),
            button: event.button,
          });
        }
      },
      { signal: abortController.signal },
    );

    canvas.addEventListener(
      "pointercancel",
      (_event) => {
        if (interaction.kind === "panning") {
          releasePointerCapture(canvas, interaction.pointerId);
          interaction = IDLE_INTERACTION;
          return;
        }

        if (
          (interaction.kind === "selecting" ||
            interaction.kind === "rectCreating") &&
          batch
        ) {
          const cancelledInteraction = interaction;
          interaction = IDLE_INTERACTION;

          if (cancelledInteraction.kind === "rectCreating") {
            pendingToolReset = ToolMode.select;
          }
          releasePointerCapture(canvas, cancelledInteraction.pointerId);
          batch.events.push({ type: "pointer_cancel" });
        }
      },
      { signal: abortController.signal },
    );

    canvas.addEventListener(
      "wheel",
      (event) => {
        if (!batch) {
          return;
        }

        batch.events.push({
          type: "camera_zoom_at_screen_point",
          pivot_px: toCanvasPoint(canvas, event),
          zoom_multiplier: Math.exp(-event.deltaY * 0.0015),
        });
        event.preventDefault();
      },
      { signal: abortController.signal, passive: false },
    );

    onCleanup(() => {
      abortController.abort();
    });
  });

  return {
    clearBatchEvents,
    ensureBatch,
    pushEvent,
    getBatch,
    isPanning,
    isSpaceDown,
    syncTool,
  };
}
