use std::{
    io::{self, Read, Write},
    path::Path,
};

use super::bitstream::{Bitstream, BitstreamWriter};
use thiserror::Error;

pub struct Plane<T> {
    pub data: T,
    pub width: usize,
    pub height: usize,
    pub sample_stride: usize,
    pub row_stride: usize,
}

impl<T: AsRef<[u16]>> Plane<T> {
    pub fn sample(&self, col: usize, row: usize) -> u16 {
        self.data.as_ref()[row * self.row_stride + col * self.sample_stride]
    }
}

pub trait Codec {
    fn encode<T: AsRef<[u16]>, W: Write>(plane: &Plane<T>, dest: W) -> io::Result<()>;
    fn decode<T: AsMut<[u16]>, R: Read>(source: R, plane: &mut Plane<T>) -> io::Result<()>;
}

#[derive(Error, Debug)]
pub enum FrameOpenError {
    #[error(transparent)]
    IO(#[from] io::Error),
    #[error(transparent)]
    TiffError(#[from] tiff::TiffError),
    #[error(transparent)]
    PngError(#[from] png::DecodingError),
    #[error("unsupported color type: {0:?}")]
    UnsupportedColorType(tiff::ColorType),
    #[error("unsupported sample type")]
    UnsupportedSampleType,
}

#[derive(PartialEq)]
pub struct RGB48Frame {
    pub data: Vec<u16>,
    pub width: usize,
    pub height: usize,
}

impl RGB48Frame {
    pub fn from_png<P: AsRef<Path>>(path: P) -> Result<Self, FrameOpenError> {
        let decoder = png::Decoder::new(std::fs::File::open(path)?);
        let (info, mut reader) = decoder.read_info()?;
        let mut data = vec![0; info.buffer_size()];
        reader.next_frame(&mut data)?;
        let data = data.iter().map(|&x| x as u16).collect();
        Ok(RGB48Frame {
            data,
            width: info.width as _,
            height: info.height as _,
        })
    }

    pub fn from_tiff<P: AsRef<Path>>(path: P) -> Result<Self, FrameOpenError> {
        let f = std::fs::File::open(path)?;
        let mut dec =
            tiff::decoder::Decoder::new(f)?.with_limits(tiff::decoder::Limits::unlimited());
        let (width, height) = dec.dimensions()?;

        Ok(match dec.colortype()? {
            tiff::ColorType::RGB(16) => match dec.read_image()? {
                tiff::decoder::DecodingResult::U16(data) => RGB48Frame {
                    data,
                    width: width as _,
                    height: height as _,
                },
                _ => return Err(FrameOpenError::UnsupportedSampleType),
            },
            color_type => return Err(FrameOpenError::UnsupportedColorType(color_type)),
        })
    }

    pub fn planes(&self) -> Vec<Plane<&[u16]>> {
        let n_planes = self.data.len() / (self.width * self.height);
        return (0..n_planes)
            .map(|plane| Plane {
                data: &self.data[plane..],
                width: self.width,
                height: self.height,
                row_stride: n_planes * self.width,
                sample_stride: n_planes,
            })
            .collect();
    }

    pub fn encode<C: Codec, W: Write>(&self, mut dest: W) -> io::Result<()> {
        {
            let mut bitstream = BitstreamWriter::new(&mut dest);
            bitstream.write_bits((self.planes().len() - 1) as _, 2)?;
            bitstream.flush()?;
        }
        for plane in self.planes() {
            C::encode(&plane, &mut dest)?;
        }
        Ok(())
    }

    pub fn decode<C: Codec, R: Read>(
        mut source: R,
        width: usize,
        height: usize,
    ) -> io::Result<Self> {
        let n_planes = {
            let mut bitstream = Bitstream::new(&mut source);
            (bitstream.read_bits(2)? as usize) + 1
        };
        let mut ret = Self {
            data: vec![0; width * height * n_planes],
            width,
            height,
        };
        for plane in 0..n_planes {
            C::decode(
                &mut source,
                &mut Plane {
                    data: &mut ret.data[plane..],
                    width: width,
                    height: height,
                    row_stride: n_planes * width,
                    sample_stride: n_planes,
                },
            )?;
        }
        Ok(ret)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rgb48_frame_open() {
        RGB48Frame::from_tiff("src/testdata/tears_of_steel_12130.tif").unwrap();
    }
}
