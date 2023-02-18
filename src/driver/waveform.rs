use std::fmt::Debug;

pub struct Waveform {
    data: Vec<[u8; 64]>,
}

impl Debug for Waveform {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Total {} frames:\n", self.data.len())?;
        for src in 0..16 {
            for dst in 0..16 {
                write!(f, "{:02} -> {:02}: ", src, dst)?;
                let idx = src * 16 + (15 - dst);  // 32bit big endian?
                for lut in &self.data {
                    let val2bit = (lut[idx / 4] >> ((idx % 4) * 2)) & 0x3;
                    match val2bit {
                        0 => write!(f, "-")?,
                        1 => write!(f, "↓")?,
                        2 => write!(f, "↑")?,
                        _ => write!(f, "?")?,
                    }
                }
                write!(f, "\n")?;
            }
        }
        Ok(())
    }
}

impl Waveform {
    pub fn frame_count(&self) -> usize {
        self.data.len()
    }

    pub fn new(data: &[u8]) -> anyhow::Result<Waveform> {
        assert!(data.len() % 64 == 0);
        let mut res : Vec<[u8; 64]> = Vec::with_capacity(data.len() % 64);
        for i in 0..(data.len() / 64) {
            let chunk: [u8; 64] = data[i*64..(i+1)*64].try_into().unwrap();
            if chunk.iter().any(|x| *x == 0xff) {
                break
            }
            res.push(chunk);
        }
        Ok(Waveform { data: res })
    }
}
