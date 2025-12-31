// MD4 digest and Quake-style block checksums.

#[derive(Clone, Debug)]
pub struct Md4 {
    state: [u32; 4],
    len_bits: u64,
    buffer: [u8; 64],
    buffer_len: usize,
}

impl Md4 {
    pub fn new() -> Self {
        Self {
            state: [0x67452301, 0xefcdab89, 0x98badcfe, 0x10325476],
            len_bits: 0,
            buffer: [0u8; 64],
            buffer_len: 0,
        }
    }

    pub fn update(&mut self, data: &[u8]) {
        if data.is_empty() {
            return;
        }

        self.len_bits = self.len_bits.wrapping_add((data.len() as u64) * 8);

        let mut offset = 0;
        if self.buffer_len > 0 {
            let remaining = 64 - self.buffer_len;
            if data.len() >= remaining {
                self.buffer[self.buffer_len..self.buffer_len + remaining]
                    .copy_from_slice(&data[..remaining]);
                transform(&mut self.state, &self.buffer);
                self.buffer_len = 0;
                offset += remaining;
            } else {
                self.buffer[self.buffer_len..self.buffer_len + data.len()]
                    .copy_from_slice(data);
                self.buffer_len += data.len();
                return;
            }
        }

        while offset + 64 <= data.len() {
            let block = &data[offset..offset + 64];
            let mut buf = [0u8; 64];
            buf.copy_from_slice(block);
            transform(&mut self.state, &buf);
            offset += 64;
        }

        let remaining = data.len() - offset;
        if remaining > 0 {
            self.buffer[..remaining].copy_from_slice(&data[offset..]);
            self.buffer_len = remaining;
        }
    }

    pub fn finalize(mut self) -> [u8; 16] {
        let bit_len = self.len_bits;

        let mut padding = [0u8; 64];
        padding[0] = 0x80;

        let pad_len = if self.buffer_len < 56 {
            56 - self.buffer_len
        } else {
            120 - self.buffer_len
        };
        self.update(&padding[..pad_len]);
        self.update(&bit_len.to_le_bytes());

        let mut out = [0u8; 16];
        for (i, value) in self.state.iter().enumerate() {
            out[i * 4..i * 4 + 4].copy_from_slice(&value.to_le_bytes());
        }
        out
    }
}

pub fn digest(data: &[u8]) -> [u8; 16] {
    let mut md4 = Md4::new();
    md4.update(data);
    md4.finalize()
}

pub fn block_checksum(data: &[u8]) -> u32 {
    let hash = digest(data);
    let mut words = [0u32; 4];
    for (i, word) in words.iter_mut().enumerate() {
        let start = i * 4;
        let bytes: [u8; 4] = hash[start..start + 4].try_into().unwrap();
        *word = u32::from_le_bytes(bytes);
    }
    words[0] ^ words[1] ^ words[2] ^ words[3]
}

pub fn block_full_checksum(data: &[u8]) -> [u8; 16] {
    digest(data)
}

fn transform(state: &mut [u32; 4], block: &[u8; 64]) {
    let mut x = [0u32; 16];
    for (i, chunk) in block.chunks_exact(4).enumerate() {
        x[i] = u32::from_le_bytes(chunk.try_into().unwrap());
    }

    let mut a = state[0];
    let mut b = state[1];
    let mut c = state[2];
    let mut d = state[3];

    macro_rules! round1 {
        ($a:ident, $b:ident, $c:ident, $d:ident, $k:expr, $s:expr) => {
            $a = $a
                .wrapping_add(f($b, $c, $d))
                .wrapping_add(x[$k])
                .rotate_left($s);
        };
    }

    macro_rules! round2 {
        ($a:ident, $b:ident, $c:ident, $d:ident, $k:expr, $s:expr) => {
            $a = $a
                .wrapping_add(g($b, $c, $d))
                .wrapping_add(x[$k])
                .wrapping_add(0x5a827999)
                .rotate_left($s);
        };
    }

    macro_rules! round3 {
        ($a:ident, $b:ident, $c:ident, $d:ident, $k:expr, $s:expr) => {
            $a = $a
                .wrapping_add(h($b, $c, $d))
                .wrapping_add(x[$k])
                .wrapping_add(0x6ed9eba1)
                .rotate_left($s);
        };
    }

    round1!(a, b, c, d, 0, 3);
    round1!(d, a, b, c, 1, 7);
    round1!(c, d, a, b, 2, 11);
    round1!(b, c, d, a, 3, 19);
    round1!(a, b, c, d, 4, 3);
    round1!(d, a, b, c, 5, 7);
    round1!(c, d, a, b, 6, 11);
    round1!(b, c, d, a, 7, 19);
    round1!(a, b, c, d, 8, 3);
    round1!(d, a, b, c, 9, 7);
    round1!(c, d, a, b, 10, 11);
    round1!(b, c, d, a, 11, 19);
    round1!(a, b, c, d, 12, 3);
    round1!(d, a, b, c, 13, 7);
    round1!(c, d, a, b, 14, 11);
    round1!(b, c, d, a, 15, 19);

    round2!(a, b, c, d, 0, 3);
    round2!(d, a, b, c, 4, 5);
    round2!(c, d, a, b, 8, 9);
    round2!(b, c, d, a, 12, 13);
    round2!(a, b, c, d, 1, 3);
    round2!(d, a, b, c, 5, 5);
    round2!(c, d, a, b, 9, 9);
    round2!(b, c, d, a, 13, 13);
    round2!(a, b, c, d, 2, 3);
    round2!(d, a, b, c, 6, 5);
    round2!(c, d, a, b, 10, 9);
    round2!(b, c, d, a, 14, 13);
    round2!(a, b, c, d, 3, 3);
    round2!(d, a, b, c, 7, 5);
    round2!(c, d, a, b, 11, 9);
    round2!(b, c, d, a, 15, 13);

    round3!(a, b, c, d, 0, 3);
    round3!(d, a, b, c, 8, 9);
    round3!(c, d, a, b, 4, 11);
    round3!(b, c, d, a, 12, 15);
    round3!(a, b, c, d, 2, 3);
    round3!(d, a, b, c, 10, 9);
    round3!(c, d, a, b, 6, 11);
    round3!(b, c, d, a, 14, 15);
    round3!(a, b, c, d, 1, 3);
    round3!(d, a, b, c, 9, 9);
    round3!(c, d, a, b, 5, 11);
    round3!(b, c, d, a, 13, 15);
    round3!(a, b, c, d, 3, 3);
    round3!(d, a, b, c, 11, 9);
    round3!(c, d, a, b, 7, 11);
    round3!(b, c, d, a, 15, 15);

    state[0] = state[0].wrapping_add(a);
    state[1] = state[1].wrapping_add(b);
    state[2] = state[2].wrapping_add(c);
    state[3] = state[3].wrapping_add(d);
}

#[inline]
fn f(x: u32, y: u32, z: u32) -> u32 {
    (x & y) | (!x & z)
}

#[inline]
fn g(x: u32, y: u32, z: u32) -> u32 {
    (x & y) | (x & z) | (y & z)
}

#[inline]
fn h(x: u32, y: u32, z: u32) -> u32 {
    x ^ y ^ z
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|b| format!("{:02x}", b)).collect()
    }

    #[test]
    fn md4_empty_digest_matches_reference() {
        let digest = digest(b"");
        assert_eq!(hex(&digest), "31d6cfe0d16ae931b73c59d7e0c089c0");
    }

    #[test]
    fn md4_multiple_updates_match_single_pass() {
        let mut md4 = Md4::new();
        md4.update(b"a");
        md4.update(b"bc");
        let digest = md4.finalize();
        assert_eq!(hex(&digest), "a448017aaf21d8525fc10ae87aa6729d");
    }

    #[test]
    fn block_checksum_empty_matches_reference() {
        assert_eq!(block_checksum(b""), 0xc6f640b7);
    }
}
