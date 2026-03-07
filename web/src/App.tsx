import { createSignal, onCleanup, onMount } from "solid-js";

type Point = {
	x: number;
	y: number;
};

type CameraView = {
	pan: Point;
	zoom: number;
};

const ToolMode = {
	select: "select",
	rect: "rect",
} as const;

type ToolMode = (typeof ToolMode)[keyof typeof ToolMode];

type InputEvent =
	| { type: "camera_pan_by_screen_delta"; delta_px: Point }
	| {
			type: "camera_zoom_at_screen_point";
			pivot_px: Point;
			zoom_multiplier: number;
	  }
	| { type: "pointer_down"; screen_px: Point; shift: boolean; button: number }
	| { type: "pointer_up"; screen_px: Point; button: number }
	| { type: "pointer_move"; screen_px: Point; buttons: number }
	| { type: "pointer_cancel" };

function App() {
	let canvasRef!: HTMLCanvasElement;
	let spaceDown = false;
	let isPanning = false;
	let isSelecting = false;
	let lastPt: Point | null = null;
	let batch: { events: InputEvent[]; tool: ToolMode } | null = null;

	const [ver, setVer] = createSignal("(loading)");
	const [cameraText, setCameraText] = createSignal("(no data)");
	const [toolMode, setToolMode] = createSignal<ToolMode>(ToolMode.select);

	const selectTool = (nextTool: ToolMode): void => {
		setToolMode(nextTool);
		if (batch) {
			batch.tool = nextTool;
		}
	};

	onMount(() => {
		let rafId = 0;
		let running = true;
		const abortController = new AbortController();

		window.addEventListener(
			"keydown",
			(e) => {
				if (e.code === "Space") {
					spaceDown = true;
					e.preventDefault();
				}
			},
			{ signal: abortController.signal },
		);

		window.addEventListener(
			"keyup",
			(e) => {
				if (e.code === "Space") {
					spaceDown = false;
					isPanning = false;
					lastPt = null;
					e.preventDefault();
				}
			},
			{ signal: abortController.signal },
		);

		canvasRef.addEventListener(
			"pointerdown",
			(e) => {
				if (e.button === 0 && spaceDown) {
					isPanning = true;
					lastPt = { x: e.clientX, y: e.clientY };
					canvasRef.setPointerCapture(e.pointerId);
					canvasRef.style.cursor = "grab";
					e.preventDefault();
					return;
				}

				if (e.button === 0 && batch) {
					isSelecting = true;
					canvasRef.setPointerCapture(e.pointerId);
					const rect = canvasRef.getBoundingClientRect();
					batch.events.push({
						type: "pointer_down",
						screen_px: { x: e.clientX - rect.left, y: e.clientY - rect.top },
						shift: e.shiftKey,
						button: e.button,
					});
				}
			},
			{ signal: abortController.signal },
		);

		canvasRef.addEventListener(
			"pointermove",
			(e) => {
				if (isPanning && lastPt && batch) {
					const dx = e.clientX - lastPt.x;
					const dy = e.clientY - lastPt.y;

					batch.events.push({
						type: "camera_pan_by_screen_delta",
						delta_px: { x: dx, y: dy },
					});
					canvasRef.style.cursor = "grabbing";

					lastPt = { x: e.clientX, y: e.clientY };
					return;
				}

				if (batch) {
					const rect = canvasRef.getBoundingClientRect();
					batch.events.push({
						type: "pointer_move",
						screen_px: { x: e.clientX - rect.left, y: e.clientY - rect.top },
						buttons: e.buttons,
					});
				}
			},
			{ signal: abortController.signal },
		);

		canvasRef.addEventListener(
			"pointerup",
			(e) => {
				if (isPanning) {
					isPanning = false;
					lastPt = null;

					canvasRef.releasePointerCapture(e.pointerId);
					return;
				}

				if (isSelecting && batch) {
					isSelecting = false;
					canvasRef.releasePointerCapture(e.pointerId);
					const rect = canvasRef.getBoundingClientRect();
					batch.events.push({
						type: "pointer_up",
						screen_px: { x: e.clientX - rect.left, y: e.clientY - rect.top },
						button: e.button,
					});
				}
			},
			{ signal: abortController.signal },
		);

		canvasRef.addEventListener(
			"pointercancel",
			(e) => {
				if (isPanning) {
					isPanning = false;
					lastPt = null;
					canvasRef.releasePointerCapture(e.pointerId);
					return;
				}

				if (isSelecting && batch) {
					isSelecting = false;
					canvasRef.releasePointerCapture(e.pointerId);
					batch.events.push({
						type: "pointer_cancel",
					});
				}
			},
			{ signal: abortController.signal },
		);

		canvasRef.addEventListener(
			"wheel",
			(e) => {
				if (!batch) return;
				const rect = canvasRef.getBoundingClientRect();
				const pivot = { x: e.clientX - rect.left, y: e.clientY - rect.top };
				const zoomMultiplier = Math.exp(-e.deltaY * 0.0015);
				batch.events.push({
					type: "camera_zoom_at_screen_point",
					pivot_px: pivot,
					zoom_multiplier: zoomMultiplier,
				});
				e.preventDefault();
			},
			{ signal: abortController.signal, passive: false },
		);

		(async () => {
			const m = await import("./wasm/app_wasm/app_wasm");
			await m.default();
			setVer(m.version());

			const app = await m.App.new(canvasRef);
			batch = { events: [], tool: toolMode() };

			const frame = () => {
				if (!running) return;
				try {
					const out = app.tick(batch) as { camera: CameraView; cursor: string };
					if (batch) {
						batch.events.length = 0;
					}

					const pan = out.camera.pan;
					const zoom = out.camera.zoom;
					setCameraText(
						`pan=(${pan.x.toFixed(2)}, ${pan.y.toFixed(2)}), zoom=${zoom.toFixed(3)}`,
					);

					const cursorMap: Record<string, string> = {
						default: "default",
						resize_tl_br: "nwse-resize",
						resize_tr_bl: "nesw-resize",
						move: "move",
						crosshair: "crosshair",
						pan: "grab",
						panning: "grabbing",
					};
					canvasRef.style.cursor = isPanning
						? "grabbing"
						: spaceDown
							? "grab"
							: cursorMap[out.cursor] ?? "default";
				} catch (err) {
					setCameraText(`tick error: ${String(err)}`);
				}

				rafId = requestAnimationFrame(frame);
			};

			rafId = requestAnimationFrame(frame);
		})();

		onCleanup(() => {
			running = false;
			if (rafId) {
				cancelAnimationFrame(rafId);
			}
			abortController.abort();
		});
	});

	return (
		<div class="min-h-screen bg-stone-100 px-4 py-6 text-stone-900 sm:px-6">
			<div class="mx-auto flex max-w-6xl flex-col gap-4">
				<div class="flex flex-wrap items-center justify-between gap-3 rounded-2xl border border-stone-300 bg-white/90 px-4 py-3 shadow-sm backdrop-blur">
					<div>
						<p class="text-sm font-medium uppercase tracking-[0.2em] text-stone-500">
							Digma
						</p>
						<p class="text-sm text-stone-600">wasm version: {ver()}</p>
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
