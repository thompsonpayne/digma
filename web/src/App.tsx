import { createMemo, createSignal } from "solid-js";

import { createCanvasInputController } from "./createCanvasInputController";
import { createWasmApp } from "./createWasmApp";
import { ToolMode } from "./editorTypes";
import type { ToolMode as ToolModeValue } from "./editorTypes";

function App() {
	let canvasRef!: HTMLCanvasElement;

	const [toolMode, setToolMode] = createSignal<ToolModeValue>(ToolMode.select);

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

				<div class="flex flex-wrap items-center gap-3 rounded-2xl border border-stone-300 bg-white px-4 py-3 shadow-sm">
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
