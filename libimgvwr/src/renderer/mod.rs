use image::imageops::FilterType;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FilterMethod {
    Nearest,
    Triangle,
    CatmullRom,
    Gaussian,
    #[default]
    Lanczos3,
}

impl From<FilterMethod> for FilterType {
    fn from(f: FilterMethod) -> FilterType {
        match f {
            FilterMethod::Nearest => FilterType::Nearest,
            FilterMethod::Triangle => FilterType::Triangle,
            FilterMethod::CatmullRom => FilterType::CatmullRom,
            FilterMethod::Gaussian => FilterType::Gaussian,
            FilterMethod::Lanczos3 => FilterType::Lanczos3,
        }
    }
}
