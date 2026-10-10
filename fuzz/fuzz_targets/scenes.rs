#![no_main]

libfuzzer_sys::fuzz_target!(|data: &[u8]| kui_fuzz::scenes::run(data));
