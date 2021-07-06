use super::{
    bitstream::{Bitstream, BitstreamWriter},
    frame::{self, Plane},
};
use std::io::{Read, Result, Write};

pub struct Codec;

pub fn fixed_prediction(a: u16, b: u16, c: u16) -> i32 {
    let min_a_b = a.min(b);
    let max_a_b = a.max(b);
    if c >= max_a_b {
        min_a_b as _
    } else if c <= min_a_b {
        max_a_b as _
    } else {
        a as i32 + b as i32 - c as i32
    }
}

pub fn encode_value<T: Write>(k: u32, x: i32, dest: &mut BitstreamWriter<T>) -> Result<()> {
    let x = ((x >> 30) ^ (2 * x)) as u32;
    let high_bits = x >> k;
    dest.write_bits(1, (high_bits + 1) as _)?;
    dest.write_bits((x & ((1 << k) - 1)) as _, k as _)?;
    Ok(())
}

pub fn decode_value<T: Read>(k: u32, source: &mut Bitstream<T>) -> Result<i32> {
    let mut high_bits = 0;
    while source.read_bits(1)? == 0 {
        high_bits += 1;
    }
    let x = (high_bits << k) | source.read_bits(k as _)? as u32;
    Ok((x as i32 >> 1) ^ ((x << 31) as i32 >> 31))
}

pub fn k(a: u16, b: u16, c: u16, d: u16) -> u32 {
    let activity_level =
        (d as i32 - b as i32).abs() + (b as i32 - c as i32).abs() + (c as i32 - a as i32).abs();
    let mut k = 0;
    while (3 << k) < activity_level {
        k += 1;
    }
    k
}

impl frame::Codec for Codec {
    fn encode<T: AsRef<[u16]>, W: Write>(plane: &Plane<T>, dest: W) -> Result<()> {
        let mut bitstream = BitstreamWriter::new(dest);
        let data = plane.data.as_ref();

        let mut b = 0;
        for row in 0..plane.height {
            let mut a = 0;
            let mut c = 0;
            for col in 0..plane.width {
                let x = data[row * plane.row_stride + col * plane.sample_stride];
                let d = if row > 0 && col + 1 < plane.width {
                    data[(row - 1) * plane.row_stride + (col + 1) * plane.sample_stride]
                } else {
                    0
                };

                let prediction = fixed_prediction(a, b, c);
                let prediction_residual = x as i32 - prediction;

                encode_value(k(a, b, c, d), prediction_residual, &mut bitstream)?;

                c = b;
                b = d;
                a = x;
            }
            b = data[row * plane.row_stride];
        }

        bitstream.flush()
    }

    fn decode<T: AsMut<[u16]>, R: Read>(source: R, plane: &mut Plane<T>) -> Result<()> {
        let mut bitstream = Bitstream::new(source);
        let data = plane.data.as_mut();

        let mut b = 0;
        for row in 0..plane.height {
            let mut a = 0;
            let mut c = 0;
            for col in 0..plane.width {
                let d = if row > 0 && col + 1 < plane.width {
                    data[(row - 1) * plane.row_stride + (col + 1) * plane.sample_stride]
                } else {
                    0
                };

                let prediction = fixed_prediction(a, b, c);
                let prediction_residual = decode_value(k(a, b, c, d), &mut bitstream)?;

                let x = (prediction + prediction_residual) as u16;
                data[row * plane.row_stride + col * plane.sample_stride] = x;

                c = b;
                b = d;
                a = x;
            }
            b = data[row * plane.row_stride];
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{super::frame::RGB48Frame, *};

    #[test]
    fn test_encode_decode_value() {
        let mut buf = Vec::new();
        for k in 0..10 {
            for &x in [-38368, -10, -1, 0, 1, 2, 3, 4, 5, 6, 38368, 38369].iter() {
                buf.clear();
                {
                    let mut dest = BitstreamWriter::new(&mut buf);
                    encode_value(k, x, &mut dest).unwrap();
                    dest.flush().unwrap();
                }
                let mut bitstream = Bitstream::new(&*buf);
                let decoded = decode_value(k, &mut bitstream).unwrap();
                assert_eq!(
                    x, decoded,
                    "k = {}, x = {}, roundtripped = {}",
                    k, x, decoded
                );
            }
        }
    }

    #[test]
    fn test_codec_12131() {
        let frame = RGB48Frame::open("src/testdata/tears_of_steel_12130.tif").unwrap();
        assert_eq!(frame.data.len(), 4096 * 1714 * 3); // 42,123,264 bytes uncompressed

        let mut encoded = Vec::new();
        frame.encode::<Codec, _>(&mut encoded).unwrap();
        assert_eq!(encoded.len(), 25526583);

        let decoded = RGB48Frame::decode::<Codec, _>(&*encoded, frame.width, frame.height).unwrap();
        assert_eq!(frame == decoded, true);
    }

    #[test]
    fn test_codec_12209() {
        let frame = RGB48Frame::open("src/testdata/tears_of_steel_12209.tif").unwrap();
        assert_eq!(frame.data.len(), 4096 * 1714 * 3); // 42,123,264 bytes uncompressed

        let mut encoded = Vec::new();
        frame.encode::<Codec, _>(&mut encoded).unwrap();
        assert_eq!(encoded.len(), 28270586);

        let decoded = RGB48Frame::decode::<Codec, _>(&*encoded, frame.width, frame.height).unwrap();
        assert_eq!(frame == decoded, true);
    }
}
