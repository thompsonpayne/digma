use engine::{Engine, EngineOutput, InputBatch};
use renderer_wgpu::Renderer;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn version() -> String {
    "0.1.0".to_string()
}

#[wasm_bindgen]
pub struct App {
    engine: Engine,
    renderer: Renderer,
}

#[wasm_bindgen]
impl App {
    #[wasm_bindgen]
    pub async fn new(canvas: web_sys::HtmlCanvasElement) -> Result<App, JsValue> {
        let renderer = Renderer::new(canvas).await?;
        Ok(App {
            engine: Engine::new(),
            renderer,
        })
    }

    #[wasm_bindgen]
    pub fn tick(&mut self, input_batch: JsValue) -> Result<JsValue, JsValue> {
        let batch: InputBatch = serde_wasm_bindgen::from_value(input_batch)
            .map_err(|e| JsValue::from_str(&format!("Invalid InputBatch: {e}")))?;

        let out: EngineOutput = self.engine.tick(&batch);
        self.renderer.render(&out.camera)?;
        serde_wasm_bindgen::to_value(&out).map_err(|e| e.into())
    }
}
