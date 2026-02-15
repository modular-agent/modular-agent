#![recursion_limit = "256"]

pub mod common;

mod suites {
    mod counter_test;
    mod external_test;
    mod modular_agent_test;
    mod preset_test;
    mod var_disabled_test;
    mod var_test;
}
