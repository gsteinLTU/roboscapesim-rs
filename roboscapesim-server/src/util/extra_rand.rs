use rand::{prelude::Distribution, Rng};

use super::util::bytes_to_hex_string;

/// Allows for getting a random distribution of uppercase hex characters (0-9 and A-F)
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

pub fn generate_random_mac_address() -> [u8; 6]
{
    let mut result = [0u8; 6];

    for b in  &mut result
    {
        *b = rand::rng().random();
    }

    // Not needed for RoboScape, but set MAC to be locally administered and unicast
    result[0] &= 0b11111110;
    result[0] |= 0b00000010;

    // Check last four digits for e and numbers
    let last_four: Vec<char> = bytes_to_hex_string(&result)[8..].to_owned().chars().collect();
    let last_four_digit_count = last_four.iter().filter(|c| char::is_ascii_digit(c)).count();

    // Prevent NetsBlox leading zero truncation
    if last_four.first().unwrap() == &'0' && last_four_digit_count == 4
    {
        result[4] |= 0b00010001;
    }

    // Accidental float prevention
    if last_four.contains(&'e') && last_four_digit_count == 3
    {
        result[4] |= 0b10001000;
    }

    result
}