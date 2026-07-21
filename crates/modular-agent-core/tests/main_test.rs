#![recursion_limit = "256"]

pub mod common;

mod suites {
    mod call_tool_message_test;
    mod call_tools_test;
    mod cancel_test;
    mod counter_test;
    mod event_test;
    mod external_test;
    mod modular_agent_test;
    mod preset_name_test;
    mod preset_test;
    mod preset_tool_test;
    mod var_disabled_test;
    mod var_test;
}
