use std::fmt::Debug;

pub struct Waveform {
    data: Vec<[u8; 64]>,
}

#[derive(Clone, Copy)]
pub enum Action {
    Keep,
    Down,
    Up,
}

impl Debug for Waveform {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Total {} frames:\n", self.data.len())?;
        for src in 0..16 {
            for dst in 0..16 {
                write!(f, "{:02} -> {:02}: ", src, dst)?;
                for action in self.get(src, dst) {
                    match action {
                        Action::Keep => write!(f, "-")?,
                        Action::Down => write!(f, "v")?,
                        Action::Up => write!(f, "^")?,
                    }
                }
                write!(f, "\n")?;
            }
        }
        Ok(())
    }
}

fn index(src: u8, dst: u8) -> usize {
    src as usize * 16 + (15 - dst as usize)  // 32bit big endian?
}

fn is_invalid_byte(val: u8) -> bool {
    (val & 0xc0) == 0xc0 || (val & 0x30) == 0x30 || (val & 0x0c) == 0x0c || (val & 0x03) == 0x03
}

impl Waveform {
    pub fn data(&self) -> &[u8] {
        unsafe {
            std::slice::from_raw_parts(self.data.as_ptr() as *const u8, self.frame_count() * 64)
        }
    }

    pub fn get(&self, src: u8, dst: u8) -> Vec<Action> {
        let idx = index(src, dst);
        self.data.iter().map(|lut| {
            match (lut[idx / 4] >> ((idx % 4) * 2)) & 0x3 {
                0 => Action::Keep,
                1 => Action::Down,
                2 => Action::Up,
                _ => panic!("should not reach here"),
            }
        }).collect()
    }

    pub fn set(&mut self, src: u8, dst: u8, actions: &[Action]) {
        assert_eq!(actions.len(), self.frame_count());
        let idx = index(src, dst);
        for (i, action) in actions.iter().enumerate() {
            let val = match *action {
                Action::Keep => 0,
                Action::Down => 1,
                Action::Up => 2,
            };
            self.data[i][idx / 4] &= !(0x3 << ((idx % 4) * 2));
            self.data[i][idx / 4] |= val << ((idx % 4) * 2);
        }
    }

    pub fn frame_count(&self) -> usize {
        self.data.len()
    }

    pub fn new(data: &[u8]) -> anyhow::Result<Waveform> {
        assert!(data.len() % 64 == 0);
        let mut res : Vec<[u8; 64]> = Vec::with_capacity(data.len() % 64);
        for i in 0..(data.len() / 64) {
            let chunk: [u8; 64] = data[i*64..(i+1)*64].try_into().unwrap();
            if chunk.iter().all(|x| *x == 0xff) {
                break
            }
            if chunk.iter().any(|x| is_invalid_byte(*x)) {
                anyhow::bail!("Invalid waveform data");
            }
            res.push(chunk);
        }
        Ok(Waveform { data: res })
    }
}
