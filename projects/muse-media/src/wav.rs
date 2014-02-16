use anyhow::Result;
use std::{fs, path::Path};
pub fn write_wav(path: &Path, seconds: f64, rate: u32) -> Result<()> {
    let count = (seconds * rate as f64) as u32;
    let mut bytes = Vec::with_capacity(44 + count as usize * 2);
    let data = count * 2;
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&(36 + data).to_le_bytes());
    bytes.extend_from_slice(b"WAVEfmt ");
    bytes.extend_from_slice(&16u32.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes());
    bytes.extend_from_slice(&rate.to_le_bytes());
    bytes.extend_from_slice(&(rate * 2).to_le_bytes());
    bytes.extend_from_slice(&2u16.to_le_bytes());
    bytes.extend_from_slice(&16u16.to_le_bytes());
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&data.to_le_bytes());
    for index in 0..count {
        let sample = ((index as f64 * 440.0 * std::f64::consts::TAU / rate as f64).sin() * 1600.0) as i16;
        bytes.extend_from_slice(&sample.to_le_bytes())
    }
    fs::write(path, bytes)?;
    Ok(())
}
