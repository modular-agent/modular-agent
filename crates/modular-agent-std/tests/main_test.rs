#![recursion_limit = "256"]

extern crate modular_agent_std;

mod suites {
    mod array_test;
    mod data_test;
    mod filter_test;
    mod input_test;
    mod sequence_test;
    mod string_test;
    mod time_test;
    mod utils_test;
    #[cfg(feature = "watch")]
    mod watch_test;
}
