use rand::{thread_rng, Rng};
use rand::distributions::Alphanumeric;

pub fn random_string(len: usize) -> String {
    thread_rng()
        .sample_iter(&Alphanumeric)
        .take(len)
        .map(char::from)
        .collect()
}
