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

/// Resize `src` to `(dst_w, dst_h)` then optionally rotate by `rotation` degrees.
///
/// `rotation` must be a multiple of 90; any other value is treated as 0.
/// When `rotation` is 90 or 270 the caller is responsible for passing the
/// pre-swap `dst_w`/`dst_h` (i.e. the post-resize, pre-rotate dimensions).
/// Dispatches resize to the appropriate GPU path:
/// - `Lanczos3` / `CatmullRom` → two-pass separable kernel (fragment shader)
/// - All others → sampler-based blit (nearest or bilinear)
#[allow(dead_code)]
pub(crate) fn resize_blit(
    ctx: &GpuContext,
    src: &wgpu::Texture,
    dst_w: u32,
    dst_h: u32,
    filter: FilterMethod,
    rotation: u16,
) -> wgpu::Texture {
    let resized = match filter {
        FilterMethod::Lanczos3 => resize_kernel_two_pass(
            ctx, src, dst_w, dst_h,
            include_str!("shaders/lanczos3.wgsl"),
        ),
        FilterMethod::CatmullRom => resize_kernel_two_pass(
            ctx, src, dst_w, dst_h,
            include_str!("shaders/catmull_rom.wgsl"),
        ),
        _ => resize_sampler(ctx, src, dst_w, dst_h, filter),
    };

    if rotation % 360 == 0 {
        resized
    } else {
        rotate_texture(ctx, &resized, rotation)
    }
}

/// Rotate `src` by `rotation` degrees (must be 90, 180, or 270; others → identity).
///
/// For 90° and 270° the output texture dimensions are swapped relative to `src`.
/// Uses a pixel-exact `textureLoad` shader — no sampler blur.
pub(crate) fn rotate_texture(
    ctx: &GpuContext,
    src: &wgpu::Texture,
    rotation: u16,
) -> wgpu::Texture {
    let src_w = src.width();
    let src_h = src.height();

    let rot_code = match rotation % 360 {
        90  => 1u32,
        180 => 2u32,
        270 => 3u32,
        _   => 0u32,
    };
    let (out_w, out_h) = if rot_code == 1 || rot_code == 3 {
        (src_h, src_w)
    } else {
        (src_w, src_h)
    };

    let dst = ctx.device.create_texture(&wgpu::TextureDescriptor {
        label: None,
        size: wgpu::Extent3d { width: out_w, height: out_h, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT
            | wgpu::TextureUsages::COPY_SRC
            | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });

    // Uniform: { src_w, src_h, rotation, _pad } = 16 bytes
    let mut ub = [0u8; 16];
    ub[0..4].copy_from_slice(&src_w.to_ne_bytes());
    ub[4..8].copy_from_slice(&src_h.to_ne_bytes());
    ub[8..12].copy_from_slice(&rot_code.to_ne_bytes());

    let uniform_buf = ctx.device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: 16,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    ctx.queue.write_buffer(&uniform_buf, 0, &ub);

    let shader = ctx.device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: None,
        source: wgpu::ShaderSource::Wgsl(include_str!("shaders/rotate.wgsl").into()),
    });

    let bgl = ctx.device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: None,
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: false },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
        ],
    });

    let pipeline_layout = ctx.device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: None,
        bind_group_layouts: &[Some(&bgl)],
        immediate_size: 0,
    });

    let pipeline = ctx.device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
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

    let bind_group = ctx.device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: None,
        layout: &bgl,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&src_view),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                    buffer: &uniform_buf,
                    offset: 0,
                    size: None,
                }),
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

/// Sampler-based blit for `Nearest`, `Triangle`, and `Gaussian`.
fn resize_sampler(
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

/// Read back a GPU texture as a Wayland-compatible ARGB8888 byte buffer.
///
/// Copies `tex` into a CPU-visible staging buffer, maps it synchronously,
/// strips wgpu's row-alignment padding, and byte-swaps RGBA → `[B, G, R, A]`
/// (little-endian ARGB8888, matching `wl_shm::Format::Argb8888`).
#[allow(dead_code)]
pub(crate) fn readback(ctx: &GpuContext, tex: &wgpu::Texture, w: u32, h: u32) -> Vec<u8> {
    const ALIGN: u32 = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
    let row_bytes = w * 4;
    let padded_row = row_bytes.div_ceil(ALIGN) * ALIGN;
    let buf_size = (padded_row * h) as u64;

    let staging = ctx.device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: buf_size,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });

    let mut encoder = ctx
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
    encoder.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture: tex,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &staging,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(padded_row),
                rows_per_image: Some(h),
            },
        },
        wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
    );
    ctx.queue.submit(std::iter::once(encoder.finish()));

    let (sender, receiver) = std::sync::mpsc::channel();
    let slice = staging.slice(..);
    slice.map_async(wgpu::MapMode::Read, move |v| sender.send(v).unwrap());
    ctx.device.poll(wgpu::PollType::Wait { submission_index: None, timeout: None }).ok();
    receiver.recv().unwrap().unwrap();

    let raw = slice.get_mapped_range();
    let mut out = Vec::with_capacity((w * h * 4) as usize);
    for row in 0..h {
        let start = (row * padded_row) as usize;
        let row_data = &raw[start..start + row_bytes as usize];
        for chunk in row_data.chunks_exact(4) {
            // RGBA → little-endian ARGB8888: [B, G, R, A]
            out.push(chunk[2]);
            out.push(chunk[1]);
            out.push(chunk[0]);
            out.push(chunk[3]);
        }
    }
    drop(raw);
    staging.unmap();
    out
}

/// Two-pass separable kernel resize (horizontal then vertical).
///
/// Pass 1: `(src_w, src_h)` → `(dst_w, src_h)` in `Rgba16Float` (preserves
/// negative Lanczos lobes without clamping).
/// Pass 2: `(dst_w, src_h)` → `(dst_w, dst_h)` in `Rgba8Unorm` (final output,
/// clamped for SHM compatibility).
fn resize_kernel_two_pass(
    ctx: &GpuContext,
    src: &wgpu::Texture,
    dst_w: u32,
    dst_h: u32,
    shader_src: &str,
) -> wgpu::Texture {
    let src_w = src.width();
    let src_h = src.height();

    let intermediate = run_kernel_pass(
        ctx, src,
        src_w, src_h, dst_w, src_h,
        wgpu::TextureFormat::Rgba16Float,
        shader_src, "fs_horizontal",
    );

    run_kernel_pass(
        ctx, &intermediate,
        dst_w, src_h, dst_w, dst_h,
        wgpu::TextureFormat::Rgba8Unorm,
        shader_src, "fs_vertical",
    )
}

/// Single render pass of a two-pass separable kernel shader.
///
/// Binds `src` as a non-filtered texture and a uniform buffer with
/// `[src_w, src_h, dst_w, dst_h]`; runs a full-screen quad with the
/// given `entry_point`; writes to a newly created texture of `out_format`.
#[allow(clippy::too_many_arguments)]
fn run_kernel_pass(
    ctx: &GpuContext,
    src: &wgpu::Texture,
    src_w: u32,
    src_h: u32,
    dst_w: u32,
    dst_h: u32,
    out_format: wgpu::TextureFormat,
    shader_src: &str,
    entry_point: &str,
) -> wgpu::Texture {
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
        format: out_format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT
            | wgpu::TextureUsages::COPY_SRC
            | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });

    // Uniform buffer: src_size (vec2<u32>) + dst_size (vec2<u32>) = 16 bytes.
    let mut ub = [0u8; 16];
    ub[0..4].copy_from_slice(&src_w.to_ne_bytes());
    ub[4..8].copy_from_slice(&src_h.to_ne_bytes());
    ub[8..12].copy_from_slice(&dst_w.to_ne_bytes());
    ub[12..16].copy_from_slice(&dst_h.to_ne_bytes());

    let uniform_buf = ctx.device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: 16,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    ctx.queue.write_buffer(&uniform_buf, 0, &ub);

    let shader = ctx.device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: None,
        source: wgpu::ShaderSource::Wgsl(shader_src.into()),
    });

    let bgl = ctx.device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: None,
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: false },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
        ],
    });

    let pipeline_layout = ctx.device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: None,
        bind_group_layouts: &[Some(&bgl)],
        immediate_size: 0,
    });

    let pipeline = ctx.device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
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
            entry_point: Some(entry_point),
            targets: &[Some(wgpu::ColorTargetState {
                format: out_format,
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

    let bind_group = ctx.device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: None,
        layout: &bgl,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&src_view),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                    buffer: &uniform_buf,
                    offset: 0,
                    size: None,
                }),
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
