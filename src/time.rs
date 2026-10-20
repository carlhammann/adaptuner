pub use web_time::{Duration, Instant};

#[cfg(target_arch = "wasm32")]
mod logical_clock {
    use super::Instant;
    use std::cell::Cell;

    thread_local! {
        static NOW: Cell<Option<Instant>> = const { Cell::new(None) };
    }

    pub fn set_now(time: Instant) {
        NOW.with(|now| now.set(Some(time)));
    }

    pub fn now() -> Instant {
        NOW.with(|now| now.get().unwrap_or_else(Instant::now))
    }
}

pub fn now() -> Instant {
    #[cfg(target_arch = "wasm32")]
    {
        logical_clock::now()
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        Instant::now()
    }
}

#[cfg(target_arch = "wasm32")]
pub fn set_now(time: Instant) {
    logical_clock::set_now(time);
}

#[cfg(all(test, target_arch = "wasm32"))]
mod tests {
    use super::*;

    #[test]
    fn logical_clock_follows_set_time() {
        let t0 = Instant::now();
        set_now(t0);

        assert_eq!(now().duration_since(t0).as_millis(), 0);

        let t1 = t0 + Duration::from_millis(500);
        set_now(t1);

        assert_eq!(now().duration_since(t0).as_millis(), 500);
    }
}
