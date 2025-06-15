use crate::config::schemes::ColorScheme;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct Options {
    /// Input file path
    pub file_path: PathBuf,
    /// Initial position in the file to jump to
    pub initial_position: usize,
    /// fps
    pub fps: f32,
    /// Color scheme to use for nucleotides
    pub color_scheme: ColorScheme,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            file_path: PathBuf::new(),
            initial_position: 0,
            fps: 25.0,
            color_scheme: ColorScheme::default(),
        }
    }
}
