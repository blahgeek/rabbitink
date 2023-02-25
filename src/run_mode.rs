use super::imgproc::DitheringMethod;
use super::driver::it8915::{DisplayMode, MemMode};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum RunMode {
    Mono(DitheringMethod),
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
            &Self::Mono(_) => DisplayMode::A2,
            &Self::Gray => DisplayMode::GL16,
        }
    }
    pub fn display_mode_slow(&self) -> DisplayMode {
        match self {
            &Self::Mono(_) => DisplayMode::DU,
            &Self::Gray => DisplayMode::GL16,
        }
    }
    pub fn mem_mode(&self) -> MemMode {
        match self {
            &Self::Mono(_) => MemMode::Mem1bpp,
            &Self::Gray => MemMode::Mem8bpp,
        }
    }

    pub fn read_from_file(path: &std::path::Path) -> anyhow::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let run_mode = match content.trim_end() {
            "mono_bayers4" => RunMode::Mono(DitheringMethod::Bayers4),
            "mono_bayers2" => RunMode::Mono(DitheringMethod::Bayers2),
            "mono_naive" => RunMode::Mono(DitheringMethod::NoDithering),
            "gray" => RunMode::Gray,
            _ => anyhow::bail!("Unsupported request: {}", content),
        };
        return Ok(run_mode)
    }
}
