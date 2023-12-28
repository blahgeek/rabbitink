use super::imgproc::DitheringMethod;
use super::driver::it8915::{DisplayMode, MemMode};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum RunMode {
    Mono(DitheringMethod),
    MonoForce8bpp(DitheringMethod),
    Gray,
}

impl Default for RunMode {
    fn default() -> Self {
        Self::Mono(DitheringMethod::Bayers4)
    }
}

impl RunMode {
    pub fn display_mode_fast(&self) -> DisplayMode {
        match self {
            &Self::Mono(_) | &Self::MonoForce8bpp(_) => DisplayMode::A2,
            &Self::Gray => DisplayMode::GL16,
        }
    }
    pub fn display_mode_slow(&self) -> DisplayMode {
        match self {
            &Self::Mono(_) | &Self::MonoForce8bpp(_) => DisplayMode::DU,
            &Self::Gray => DisplayMode::GL16,
        }
    }
    pub fn mem_mode(&self) -> MemMode {
        match self {
            &Self::Mono(_) => MemMode::Mem1bpp,
            &Self::MonoForce8bpp(_) => MemMode::Mem8bpp,
            &Self::Gray => MemMode::Mem8bpp,
        }
    }
    pub fn dithering_method(&self) -> Option<DitheringMethod> {
        match self {
            &Self::Mono(v) | &Self::MonoForce8bpp(v) => Some(v),
            &Self::Gray => None,
        }
    }

    pub fn read_from_file(path: &std::path::Path) -> anyhow::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let run_mode = match content.trim_end() {
            "mono_bayers4" => RunMode::Mono(DitheringMethod::Bayers4),
            "mono_bayers2" => RunMode::Mono(DitheringMethod::Bayers2),
            "mono_naive" => RunMode::Mono(DitheringMethod::NoDithering),
            "mono_8bpp_bayers4" => RunMode::MonoForce8bpp(DitheringMethod::Bayers4),
            "mono_8bpp_bayers2" => RunMode::MonoForce8bpp(DitheringMethod::Bayers2),
            "mono_8bpp_naive" => RunMode::MonoForce8bpp(DitheringMethod::NoDithering),
            "gray" => RunMode::Gray,
            _ => anyhow::bail!("Unsupported request: {}", content),
        };
        return Ok(run_mode)
    }
}
