#[derive(Clone, Copy, Debug)]
pub enum CompressMode {
    Balanced,
    Ultra,
}

impl CompressMode {
    pub fn jpeg_quality(&self) -> u8 {
        match self {
            Self::Balanced => 80,
            Self::Ultra => 68,
        }
    }

    pub fn png_quality_bounds(&self) -> (u8, u8) {
        match self {
            Self::Balanced => (65, 80),
            Self::Ultra => (40, 65),
        }
    }

    pub fn oxipng_preset(&self) -> u8 {
        match self {
            Self::Balanced => 2,
            Self::Ultra => 4,
        }
    }

    pub fn webp_quality(&self) -> f32 {
        match self {
            Self::Balanced => 75.0,
            Self::Ultra => 60.0,
        }
    }

    pub fn avif_params(&self) -> (f32, u8) {
        // (quality, speed)
        match self {
            Self::Balanced => (65.0, 6),
            Self::Ultra => (50.0, 4),
        }
    }
}
