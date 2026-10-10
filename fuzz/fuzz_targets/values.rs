#![no_main]

libfuzzer_sys::fuzz_target!(|data: &[u8]| kui_fuzz::values::run(data));
