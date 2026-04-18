// GPU-accelerated rendering pipeline via wgpu (Vulkan preferred, GL/EGL fallback).
// Compiled only when `gpu-vulkan` or `gpu-gles` feature is enabled.

use image::DynamicImage;
use super::FilterMethod;

/// Errors produced during GPU initialisation.
#[derive(Debug)]
pub enum GpuError {
    NoAdapter,
    DeviceError(wgpu::RequestDeviceError),
}

impl std::fmt::Display for GpuError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GpuError::NoAdapter => write!(f, "no suitable GPU adapter found"),
            GpuError::DeviceError(e) => write!(f, "GPU device error: {e}"),
        }
    }
}

impl std::error::Error for GpuError {}

impl From<wgpu::RequestDeviceError> for GpuError {
    fn from(e: wgpu::RequestDeviceError) -> Self {
        GpuError::DeviceError(e)
    }
}

/// Owns the wgpu device and queue for the lifetime of the application.
pub struct GpuContext {
    #[allow(dead_code)]
    pub(crate) device: wgpu::Device,
    #[allow(dead_code)]
    pub(crate) queue: wgpu::Queue,
}

impl GpuContext {
    /// Initialise a GPU context. Returns `Err` if no suitable adapter is found;
    /// the caller should treat this as a fatal error and exit.
    pub fn new() -> Result<Self, GpuError> {
        pollster::block_on(async {
            let mut backends = wgpu::Backends::empty();
            #[cfg(feature = "gpu-vulkan")]
            {
                backends |= wgpu::Backends::VULKAN;
            }
            #[cfg(feature = "gpu-gles")]
            {
                backends |= wgpu::Backends::GL;
            }

            let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
                backends,
                ..wgpu::InstanceDescriptor::new_without_display_handle()
            });

            let adapter = instance
                .request_adapter(&wgpu::RequestAdapterOptions {
                    power_preference: wgpu::PowerPreference::HighPerformance,
                    compatible_surface: None,
                    force_fallback_adapter: false,
                })
                .await
                .map_err(|_| GpuError::NoAdapter)?;

            let info = adapter.get_info();
            tracing::info!(
                backend = ?info.backend,
                adapter = %info.name,
                "GPU adapter selected"
            );

            let (device, queue) = adapter
                .request_device(&wgpu::DeviceDescriptor::default())
                .await?;

            Ok(Self { device, queue })
        })
    }
}

/// Upload a [`DynamicImage`] to a GPU texture.
///
/// The returned texture uses `Rgba8Unorm` format and is suitable for
/// sampling in a render or compute pass.
#[allow(dead_code)]
pub(crate) fn upload_texture(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    img: &DynamicImage,
) -> wgpu::Texture {
    let rgba = img.to_rgba8();
    let (width, height) = rgba.dimensions();

    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: None,
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });

    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture: &texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        rgba.as_raw(),
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(4 * width),
            rows_per_image: Some(height),
        },
        wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
    );

    texture
}

/// Resize `src` to `(dst_w, dst_h)` on the GPU using a sampler-based blit.
///
/// `Nearest` → nearest-neighbour sampler; all other filters → bilinear.
/// `Lanczos3` and `CatmullRom` also use bilinear here; they are overridden
/// by compute shaders in step 8.5.
#[allow(dead_code)]
pub(crate) fn resize_blit(
    ctx: &GpuContext,
    src: &wgpu::Texture,
    dst_w: u32,
    dst_h: u32,
    filter: FilterMethod,
) -> wgpu::Texture {
    let filter_mode = match filter {
        FilterMethod::Nearest => wgpu::FilterMode::Nearest,
        _ => wgpu::FilterMode::Linear,
    };

    let dst = ctx.device.create_texture(&wgpu::TextureDescriptor {
        label: None,
        size: wgpu::Extent3d {
            width: dst_w,
            height: dst_h,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT
            | wgpu::TextureUsages::COPY_SRC
            | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });

    let shader = ctx
        .device
        .create_shader_module(wgpu::ShaderModuleDescriptor {
            label: None,
            source: wgpu::ShaderSource::Wgsl(
                include_str!("shaders/blit.wgsl").into(),
            ),
        });

    let bind_group_layout =
        ctx.device
            .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: None,
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float {
                                filterable: true,
                            },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(
                            wgpu::SamplerBindingType::Filtering,
                        ),
                        count: None,
                    },
                ],
            });

    let pipeline_layout =
        ctx.device
            .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: None,
                bind_group_layouts: &[Some(&bind_group_layout)],
                immediate_size: 0,
            });

    let pipeline =
        ctx.device
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: None,
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs_main"),
                    buffers: &[],
                    compilation_options: Default::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some("fs_main"),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: wgpu::TextureFormat::Rgba8Unorm,
                        blend: None,
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                    compilation_options: Default::default(),
                }),
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleStrip,
                    strip_index_format: None,
                    ..Default::default()
                },
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                multiview_mask: None,
                cache: None,
            });

    let src_view = src.create_view(&wgpu::TextureViewDescriptor::default());
    let dst_view = dst.create_view(&wgpu::TextureViewDescriptor::default());

    let sampler = ctx.device.create_sampler(&wgpu::SamplerDescriptor {
        mag_filter: filter_mode,
        min_filter: filter_mode,
        ..Default::default()
    });

    let bind_group = ctx.device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: None,
        layout: &bind_group_layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&src_view),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(&sampler),
            },
        ],
    });

    let mut encoder = ctx
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor::default());

    {
        let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: None,
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &dst_view,
                resolve_target: None,
                depth_slice: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        rpass.set_pipeline(&pipeline);
        rpass.set_bind_group(0, &bind_group, &[]);
        rpass.draw(0..4, 0..1);
    }

    ctx.queue.submit(std::iter::once(encoder.finish()));

    dst
}
