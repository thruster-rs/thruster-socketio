use rand::distributions::Alphanumeric;
use rand::{thread_rng, Rng};

pub fn generate_sid() -> String {
    thread_rng().sample_iter(&Alphanumeric).take(30).collect()
}
