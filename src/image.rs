use std::{io, path::Path};
use thiserror::Error;

pub struct Image<T> {
    pub data: T,
    pub width: usize,
    pub height: usize,
    pub sample_stride: usize,
    pub row_stride: usize,
}

impl<T: AsRef<[u8]>> Image<T> {
    pub fn sample(&self, col: usize, row: usize) -> u8 {
        self.data.as_ref()[row * self.row_stride + col * self.sample_stride]
    }

    /// Returns the PSNR in dB of the given approximation of this 8-bit image.
    pub fn psnr<U: AsRef<[u8]>>(&self, approximation: &Image<U>) -> f64 {
        let mut mse = 0.0;
        for x in 0..self.width {
            for y in 0..self.height {
                let i = self.sample(x, y) as f64;
                let k = approximation.sample(x, y) as f64;
                mse += (i - k) * (i - k);
            }
        }
        mse /= (self.width * self.height) as f64;
        10.0 * (255.0 * 255.0 / mse).log10()
    }
}

#[derive(Error, Debug)]
pub enum LoadFileError {
    #[error(transparent)]
    IO(#[from] io::Error),
    #[error(transparent)]
    ImageError(#[from] image::ImageError),
    #[error("unsupported pixel format")]
    UnsupportedPixelFormat,
}

pub fn load_monochrome_file<P: AsRef<Path>>(path: P) -> Result<Image<Vec<u8>>, LoadFileError> {
    let img = image::io::Reader::open(path)?.decode()?;

    match img {
        image::DynamicImage::ImageLuma8(img) => Ok(Image {
            width: img.width() as _,
            height: img.height() as _,
            sample_stride: 1,
            row_stride: img.width() as _,
            data: img.into_vec(),
        }),
        _ => Err(LoadFileError::UnsupportedPixelFormat),
    }
}
