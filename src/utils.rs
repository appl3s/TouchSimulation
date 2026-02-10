use rand::Rng;

const LETTER_BYTES: &str = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ";

pub fn rand_string_bytes(n: usize) -> String {
    let mut rng = rand::thread_rng();
    (0..n)
        .map(|_| {
            let idx = rng.gen_range(0..LETTER_BYTES.len());
            LETTER_BYTES.chars().nth(idx).unwrap()
        })
        .collect()
}

pub fn rand_integer_num(n: u32) -> u32 {
    let mut rng = rand::thread_rng();
    rng.gen_range(0..n)
}

pub fn rand_u16_num(n: u16) -> u16 {
    let mut rng = rand::thread_rng();
    rng.gen_range(0..n)
}

pub fn get_random_shift() -> i32 {
    let mut rng = rand::thread_rng();
    rng.gen_range(-20..21) // [-20, 20]
}