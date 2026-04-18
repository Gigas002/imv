// GPU-accelerated rendering pipeline via wgpu (Vulkan preferred, GL/EGL fallback).
// Compiled only when `gpu-vulkan` or `gpu-gles` feature is enabled.

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
