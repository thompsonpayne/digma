import { createSignal, onCleanup, onMount } from "solid-js";
import "./App.css";

type Point = {
	x: number;
	y: number;
};

type CameraView = {
	pan: Point;
	zoom: number;
};

type InputEvent =
	| { type: "camera_pan_by_screen_delta"; delta_px: Point }
	| {
			type: "camera_zoom_at_screen_point";
			pivot_px: Point;
			zoom_multiplier: number;
	  };

function App() {
	let canvasRef!: HTMLCanvasElement;
	let spaceDown = false;
	let isPanning = false;
	let lastPt: Point | null = null;
	let batch: { events: InputEvent[] } | null = null;

	const [ver, setVer] = createSignal("(loading)");
	const [cameraText, setCameraText] = createSignal("(no data)");

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
					e.preventDefault();
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
					lastPt = { x: e.clientX, y: e.clientY };
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
			batch = { events: [] };

			const frame = () => {
				if (!running) return;
				try {
					const out = app.tick(batch) as { camera: CameraView };
					if (batch) {
						batch.events.length = 0;
					}

					const pan = out.camera.pan;
					const zoom = out.camera.zoom;
					setCameraText(
						`pan=(${pan.x.toFixed(2)}, ${pan.y.toFixed(2)}), zoom=${zoom.toFixed(3)}`,
					);
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
		<div class="app">
			<div class="hud">wasm version: {ver()}</div>
			<div class="hud">{cameraText()}</div>
			<canvas
				ref={canvasRef}
				class="viewport"
				width={800}
				height={600}
			></canvas>
		</div>
	);
}

export default App;
