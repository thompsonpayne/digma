use engine::{Camera, OverlayScene, RenderScene};
use wasm_bindgen::JsValue;
use web_sys::HtmlCanvasElement;
#[cfg(target_arch = "wasm32")]
use wgpu::util::DeviceExt;

#[cfg(target_arch = "wasm32")]
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Vertex {
    pos: [f32; 2],
}

#[cfg(target_arch = "wasm32")]
const QUAD_VERTS: [Vertex; 6] = [
    Vertex { pos: [0.0, 0.0] },
    Vertex { pos: [1.0, 0.0] },
    Vertex { pos: [1.0, 1.0] },
    Vertex { pos: [0.0, 0.0] },
    Vertex { pos: [1.0, 1.0] },
    Vertex { pos: [0.0, 1.0] },
];

#[cfg(target_arch = "wasm32")]
const SHADER: &str = include_str!("shader.wgsl");

pub struct Renderer {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    pipeline: wgpu::RenderPipeline,

    vertex_buf: wgpu::Buffer,
    vertex_count: u32,

    camera_buf: wgpu::Buffer,
    camera_bind_group: wgpu::BindGroup,

    scene_instance: wgpu::Buffer,
    scene_instance_count: u32,
    scene_instance_capacity: usize,

    overlay_instance: wgpu::Buffer,
    overlay_instance_count: u32,
    overlay_instance_capacity: usize,
}

impl Renderer {
    #[cfg(target_arch = "wasm32")]
    pub async fn new(canvas: HtmlCanvasElement) -> Result<Self, JsValue> {
        let width = canvas.width().max(1);
        let height = canvas.height().max(1);

        let scene_instance = wgpu::Instance::default();
        let surface = scene_instance
            .create_surface(wgpu::SurfaceTarget::Canvas(canvas))
            .map_err(|e| JsValue::from_str(&format!("create_surface failed: {e}")))?;

        let adapter = scene_instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::LowPower,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .map_err(|e| JsValue::from_str(&format!("request_adapter failed: {e}")))?;

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("renderer_wgpu device"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::downlevel_webgl2_defaults(),
                memory_hints: wgpu::MemoryHints::default(),
                trace: wgpu::Trace::Off,
                experimental_features: wgpu::ExperimentalFeatures::default(),
            })
            .await
            .map_err(|e| JsValue::from_str(&format!("request_device failed: {e}")))?;

        let caps = surface.get_capabilities(&adapter);
        let format = caps.formats[0];

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width,
            height,
            present_mode: caps.present_modes[0],
            alpha_mode: caps.alpha_modes[0],
            desired_maximum_frame_latency: 2,
            view_formats: vec![],
        };

        surface.configure(&device, &config);

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("simple shader"),
            source: wgpu::ShaderSource::Wgsl(SHADER.into()),
        });

        let vertex_layout = wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Vertex>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x2,
                offset: 0,
                shader_location: 0,
            }],
        };

        let camera_uniform = CameraUniform {
            pan: [0.0, 0.0],
            zoom: 1.0,
            _pad0: 0.0,
            canvas: [width as f32, height as f32],
            _pad1: [0.0, 0.0],
        };

        let camera_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("camera uniform"),
            contents: bytemuck::bytes_of(&camera_uniform),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let camera_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("camera bind group layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });

        let camera_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("camera bind group"),
            layout: &camera_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: camera_buf.as_entire_binding(),
            }],
        });

        let instance_capacity = 1024;
        let instance_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("rect instance buffer"),
            size: (std::mem::size_of::<GpuRectInstance>() * instance_capacity) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let overlay_instance_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("overlay instance buffer"),
            size: (std::mem::size_of::<GpuRectInstance>() * instance_capacity) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let instance_layout = wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<GpuRectInstance>() as u64,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &[
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x2,
                    offset: 0,
                    shader_location: 1,
                },
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x2,
                    offset: 8,
                    shader_location: 2,
                },
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x4,
                    offset: 16,
                    shader_location: 3,
                },
            ],
        };

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("simple pipeline layout"),
            bind_group_layouts: &[&camera_bind_group_layout],
            immediate_size: 0,
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("simple pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[vertex_layout, instance_layout],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

        let vertex_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("rect vertices"),
            contents: bytemuck::cast_slice(&QUAD_VERTS),
            usage: wgpu::BufferUsages::VERTEX,
        });

        Ok(Self {
            surface,
            device,
            queue,
            config,
            pipeline,
            vertex_buf,
            vertex_count: QUAD_VERTS.len() as u32,
            camera_buf,
            camera_bind_group,
            scene_instance: instance_buf,
            scene_instance_count: 0,
            scene_instance_capacity: instance_capacity,
            overlay_instance: overlay_instance_buf,
            overlay_instance_count: 0,
            overlay_instance_capacity: instance_capacity,
        })
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub async fn new(canvas: HtmlCanvasElement) -> Result<Self, JsValue> {
        let _ = canvas;
        Err(JsValue::from_str(
            "renderer_wgpu only supports wasm32 targets",
        ))
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        let width = width.max(1);
        let height = height.max(1);
        if self.config.width == width && self.config.height == height {
            return;
        }
        self.config.width = width;
        self.config.height = height;
        self.surface.configure(&self.device, &self.config);

        let camera_uniform = CameraUniform {
            pan: [0.0, 0.0],
            zoom: 1.0,
            _pad0: 0.0,
            canvas: [self.config.width as f32, self.config.height as f32],
            _pad1: [0.0, 0.0],
        };

        self.queue
            .write_buffer(&self.camera_buf, 0, bytemuck::bytes_of(&camera_uniform));
    }

    pub fn render(
        &mut self,
        camera: &Camera,
        scene: &RenderScene,
        overlay: &OverlayScene,
    ) -> Result<(), JsValue> {
        let camera_uniform = CameraUniform {
            pan: [camera.pan.x, camera.pan.y],
            zoom: camera.zoom,
            _pad0: 0.0,
            canvas: [self.config.width as f32, self.config.height as f32],
            _pad1: [0.0, 0.0],
        };

        self.queue
            .write_buffer(&self.camera_buf, 0, bytemuck::bytes_of(&camera_uniform));

        let frame = self
            .surface
            .get_current_texture()
            .map_err(|e| JsValue::from_str(&format!("get_current_texture failed: {e}")))?;
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let instances: Vec<GpuRectInstance> = scene
            .rects
            .iter()
            .map(|r| GpuRectInstance {
                pos: r.pos,
                size: r.size,
                color: r.color,
            })
            .collect();

        let overlay_instances: Vec<GpuRectInstance> = overlay
            .rects
            .iter()
            .map(|r| GpuRectInstance {
                pos: r.pos,
                size: r.size,
                color: r.color,
            })
            .collect();

        let needed = instances.len();
        let overlay_needed = overlay_instances.len();

        if needed > self.scene_instance_capacity {
            let new_capacity = needed.next_power_of_two();
            self.scene_instance_capacity = new_capacity;

            self.scene_instance = self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("rect instance buffer"),
                size: (std::mem::size_of::<GpuRectInstance>() * new_capacity) as u64,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            })
        }

        self.queue
            .write_buffer(&self.scene_instance, 0, bytemuck::cast_slice(&instances));
        self.scene_instance_count = instances.len() as u32;

        if overlay_needed > self.overlay_instance_capacity {
            let new_capacity = overlay_needed.next_power_of_two();
            self.overlay_instance_capacity = new_capacity;

            self.overlay_instance = self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("overlay instance buffer"),
                size: (std::mem::size_of::<GpuRectInstance>() * new_capacity) as u64,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            })
        }

        self.queue.write_buffer(
            &self.overlay_instance,
            0,
            bytemuck::cast_slice(&overlay_instances),
        );
        self.overlay_instance_count = overlay_instances.len() as u32;

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("render encoder"),
            });

        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("render pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.08,
                            g: 0.09,
                            b: 0.12,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
                multiview_mask: None,
            });

            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &self.camera_bind_group, &[]);
            pass.set_vertex_buffer(0, self.vertex_buf.slice(..));
            pass.set_vertex_buffer(1, self.scene_instance.slice(..));
            pass.draw(0..self.vertex_count, 0..self.scene_instance_count);
        }

        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("overlay pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
                multiview_mask: None,
            });

            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &self.camera_bind_group, &[]);
            pass.set_vertex_buffer(0, self.vertex_buf.slice(..));
            pass.set_vertex_buffer(1, self.overlay_instance.slice(..));
            pass.draw(0..self.vertex_count, 0..self.overlay_instance_count);
        }

        self.queue.submit(Some(encoder.finish()));
        frame.present();
        Ok(())
    }
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct CameraUniform {
    pan: [f32; 2],
    zoom: f32,
    _pad0: f32,
    canvas: [f32; 2],
    _pad1: [f32; 2],
}

#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct GpuRectInstance {
    pos: [f32; 2],
    size: [f32; 2],
    color: [f32; 4],
}
