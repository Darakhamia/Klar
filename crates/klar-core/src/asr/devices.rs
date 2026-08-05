//! What ggml actually found on this machine, as opposed to what was compiled.
//!
//! [`Backend::compiled`](super::Backend::compiled) answers a build question:
//! which accelerator this binary carries code for. It cannot answer the
//! question that decides whether Klar is usable — whether a device that
//! accelerator can talk to is present. The two come apart in the exact case
//! this module exists for: a CUDA build on a machine with an AMD card, where
//! the CUDA runtime loads, finds no NVIDIA device, and ggml falls back to the
//! CPU. Everything keeps working and every stage takes ten times as long, and
//! nothing in the log said so.
//!
//! ggml keeps a registry of the devices its compiled backends found. Reading it
//! turns "built for cuda" into "built for cuda, found no GPU", which is a
//! sentence that can be shown to somebody.

use super::Backend;
use std::ffi::CStr;

/// What kind of processor a registered device is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum DeviceKind {
    Cpu,
    /// A discrete card.
    Gpu,
    /// Integrated graphics — real acceleration, and a fraction of the
    /// throughput of a discrete card. Worth telling apart when explaining a
    /// disappointing measurement.
    IntegratedGpu,
    /// Something else ggml can offload to.
    Accelerator,
    /// A device type from a newer ggml than this one knows about.
    Unknown,
}

impl DeviceKind {
    const fn from_raw(raw: u32) -> Self {
        match raw {
            whisper_rs::whisper_rs_sys::ggml_backend_dev_type_GGML_BACKEND_DEVICE_TYPE_CPU => {
                Self::Cpu
            }
            whisper_rs::whisper_rs_sys::ggml_backend_dev_type_GGML_BACKEND_DEVICE_TYPE_GPU => {
                Self::Gpu
            }
            whisper_rs::whisper_rs_sys::ggml_backend_dev_type_GGML_BACKEND_DEVICE_TYPE_IGPU => {
                Self::IntegratedGpu
            }
            whisper_rs::whisper_rs_sys::ggml_backend_dev_type_GGML_BACKEND_DEVICE_TYPE_ACCEL => {
                Self::Accelerator
            }
            _ => Self::Unknown,
        }
    }

    /// Whether work offloaded here runs on something other than the CPU.
    ///
    /// `Unknown` counts: a device type from a newer ggml is far more likely to
    /// be a new kind of accelerator than a second kind of CPU, and the cost of
    /// being wrong is a missing warning rather than a false one.
    pub const fn is_accelerator(self) -> bool {
        !matches!(self, Self::Cpu)
    }
}

/// One device from ggml's registry.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Device {
    /// ggml's own handle for it: `CUDA0`, `Vulkan0`, `CPU`.
    pub name: String,
    /// The human name, when the backend knows one: `NVIDIA GeForce RTX 4070`,
    /// `AMD Radeon RX 7800 XT`. Falls back to `name`.
    pub description: String,
    pub kind: DeviceKind,
    /// Total device memory in bytes, when the backend reports it.
    pub memory: Option<u64>,
}

impl std::fmt::Display for Device {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} [{}]", self.description, self.name)
    }
}

/// Every device ggml registered, in the order it registered them.
///
/// ggml builds this registry when the library loads, so this is safe to call
/// before any whisper context exists — which is the point, since the answer
/// decides what to tell somebody about a model that has not loaded yet.
pub fn devices() -> Vec<Device> {
    // SAFETY: the registry is populated during static initialisation of the
    // linked ggml and is immutable afterwards. `dev_get` returns a pointer ggml
    // owns for the life of the process; the strings likewise. Indices below the
    // reported count are the documented valid range.
    let count = unsafe { whisper_rs::whisper_rs_sys::ggml_backend_dev_count() };

    (0..count)
        .filter_map(|index| unsafe { device_at(index) })
        .collect()
}

/// # Safety
///
/// `index` must be less than `ggml_backend_dev_count()`.
unsafe fn device_at(index: usize) -> Option<Device> {
    let device = unsafe { whisper_rs::whisper_rs_sys::ggml_backend_dev_get(index) };
    if device.is_null() {
        return None;
    }

    let name = unsafe { owned(whisper_rs::whisper_rs_sys::ggml_backend_dev_name(device)) }?;
    let description = unsafe {
        owned(whisper_rs::whisper_rs_sys::ggml_backend_dev_description(
            device,
        ))
    }
    .unwrap_or_else(|| name.clone());
    let kind =
        DeviceKind::from_raw(unsafe { whisper_rs::whisper_rs_sys::ggml_backend_dev_type(device) });

    let mut free = 0usize;
    let mut total = 0usize;
    unsafe {
        whisper_rs::whisper_rs_sys::ggml_backend_dev_memory(device, &raw mut free, &raw mut total);
    }

    Some(Device {
        name,
        description,
        kind,
        memory: (total > 0).then_some(total as u64),
    })
}

/// # Safety
///
/// `raw` must be null or a NUL-terminated string valid for the duration of the
/// call. The result is copied, so the pointer is not retained.
unsafe fn owned(raw: *const std::os::raw::c_char) -> Option<String> {
    if raw.is_null() {
        return None;
    }
    let text = unsafe { CStr::from_ptr(raw) }
        .to_string_lossy()
        .trim()
        .to_owned();
    (!text.is_empty()).then_some(text)
}

/// What this build asked for, and what it got.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Acceleration {
    /// The backend compiled into this binary.
    pub compiled: Backend,
    /// The device the work will actually land on, when there is one.
    pub device: Option<Device>,
    /// Every device found, for the log and for `doctor`.
    pub found: Vec<Device>,
}

impl Acceleration {
    /// Ask ggml.
    pub fn probe() -> Self {
        let found = devices();
        // The first accelerator wins. ggml registers its GPU backends before
        // the CPU one and offloads to the first that can take the graph, so
        // "first accelerator in the registry" is the same choice whisper.cpp
        // will make rather than a guess at the fastest card.
        let device = found.iter().find(|d| d.kind.is_accelerator()).cloned();
        Self {
            compiled: Backend::compiled(),
            device,
            found,
        }
    }

    /// Whether speech will be recognised on something faster than the CPU.
    pub const fn accelerated(&self) -> bool {
        self.device.is_some()
    }

    /// One line for a person: what is doing the work.
    pub fn summary(&self) -> String {
        match &self.device {
            Some(device) => format!("{} ({})", device.description, self.compiled),
            None => "CPU only".to_owned(),
        }
    }

    /// What is wrong, written for somebody who did not choose this build.
    ///
    /// `None` when the build and the machine agree. The two failures worth
    /// separating are a CPU build (the user downloaded the wrong installer, or
    /// there is only one) and a GPU build that found no GPU (the user
    /// downloaded an installer for somebody else's card), because the remedy
    /// differs and "it is slow" does not distinguish them.
    pub fn warning(&self) -> Option<String> {
        if self.accelerated() {
            return None;
        }

        Some(match self.compiled {
            Backend::Cpu => "Speech is being recognised on the CPU, which is roughly ten \
                 times slower than a graphics card. This build carries no GPU \
                 support at all — the Vulkan build works on AMD, Intel and \
                 NVIDIA alike."
                .to_owned(),
            compiled => format!(
                "This build was made for {compiled}, and no {compiled} device was found on \
                 this machine, so speech is being recognised on the CPU — roughly ten times \
                 slower. The Vulkan build works on AMD, Intel and NVIDIA alike."
            ),
        })
    }

    /// Write what was found into the log, at a level matching how bad it is.
    pub fn log(&self) {
        for device in &self.found {
            tracing::info!(
                name = %device.name,
                description = %device.description,
                kind = ?device.kind,
                memory_mb = device.memory.map(|bytes| bytes / (1024 * 1024)),
                "ggml device"
            );
        }

        match self.warning() {
            Some(warning) => tracing::warn!(compiled = %self.compiled, "{warning}"),
            None => tracing::info!(
                compiled = %self.compiled,
                device = %self.summary(),
                "asr is accelerated"
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// ggml always registers a CPU device, whatever else it found. An empty
    /// registry means the enumeration itself is broken — which would make every
    /// other answer here a quiet lie rather than a wrong one.
    #[test]
    fn the_registry_is_never_empty() {
        let found = devices();
        assert!(!found.is_empty(), "ggml registered no devices at all");
        assert!(
            found.iter().any(|device| device.kind == DeviceKind::Cpu),
            "no CPU device in {found:?}"
        );
    }

    #[test]
    fn every_device_is_named() {
        for device in devices() {
            assert!(!device.name.is_empty());
            assert!(!device.description.is_empty());
        }
    }

    /// The invariant the warning depends on: it fires when, and only when, no
    /// accelerator was found. On CI that is the CPU branch; on a developer
    /// machine with a card it is the silent one. Both are correct, and asserting
    /// the pair keeps a future edit from making the warning unconditional.
    #[test]
    fn a_warning_means_no_accelerator_was_found() {
        let acceleration = Acceleration::probe();
        assert_eq!(acceleration.warning().is_none(), acceleration.accelerated());
    }

    #[test]
    fn a_cpu_build_says_it_carries_no_gpu_support() {
        let acceleration = Acceleration {
            compiled: Backend::Cpu,
            device: None,
            found: Vec::new(),
        };
        let warning = acceleration.warning().unwrap_or_default();
        assert!(warning.contains("no GPU support"), "got {warning}");
        assert!(warning.contains("Vulkan"), "got {warning}");
    }

    /// The case this module was written for: the build is fine, the machine is
    /// fine, and they are for different vendors.
    #[test]
    fn a_cuda_build_with_no_cuda_device_names_the_mismatch() {
        let acceleration = Acceleration {
            compiled: Backend::Cuda,
            device: None,
            found: Vec::new(),
        };
        let warning = acceleration.warning().unwrap_or_default();
        assert!(warning.contains("cuda"), "got {warning}");
        assert!(warning.contains("Vulkan"), "got {warning}");
    }

    #[test]
    fn a_found_device_silences_the_warning_and_names_itself() {
        let acceleration = Acceleration {
            compiled: Backend::Vulkan,
            device: Some(Device {
                name: "Vulkan0".to_owned(),
                description: "AMD Radeon RX 7800 XT".to_owned(),
                kind: DeviceKind::Gpu,
                memory: Some(16 * 1024 * 1024 * 1024),
            }),
            found: Vec::new(),
        };
        assert!(acceleration.warning().is_none());
        assert_eq!(acceleration.summary(), "AMD Radeon RX 7800 XT (vulkan)");
    }

    #[test]
    fn integrated_graphics_count_as_acceleration() {
        assert!(DeviceKind::IntegratedGpu.is_accelerator());
        assert!(DeviceKind::Unknown.is_accelerator());
        assert!(!DeviceKind::Cpu.is_accelerator());
    }
}
