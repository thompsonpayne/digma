build-wasm:
	@wasm-pack build crates/app_wasm --target web --release --out-dir ../../web/src/wasm/app_wasm
run-web:
	@pnpm -C web dev
