use arduino_hal::port::mode::{Analog, Output};
use arduino_hal::port::Pin;

pub struct Mempad {
    ab: [Pin<Analog>; 4],
    db: [Pin<Output>; 4] // ← must be PWM
}

fn blocking_read() {
    let curr = ab[0].bxor(db.iter().flush());
}
