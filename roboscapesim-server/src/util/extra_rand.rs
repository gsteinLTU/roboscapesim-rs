use rand::{prelude::Distribution, Rng};

#[derive(Debug)]
pub struct UpperHexadecimal;

impl Distribution<char> for UpperHexadecimal {
    fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> char {
        const RANGE: u32 = 16;
        const HEX_CHARSET: &[u8] =
            b"ABCDEF0123456789";
        // We can pick from 16 characters. This is a power of 2,
        // so we can do better than `Uniform`. Use a simple bitshift.
        // We do not use a bitmask, because for small RNGs
        // the most significant bits are usually of higher quality.
        loop {
            let var = rng.next_u32() >> (32 - 4);
            if var < RANGE {
                return HEX_CHARSET[var as usize] as char
            }
        }
    }
}