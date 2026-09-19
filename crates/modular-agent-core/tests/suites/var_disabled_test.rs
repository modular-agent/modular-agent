extern crate modular_agent_core as ma;

use ma::{Error, Value, test_utils};
use serial_test::serial;

#[serial(external_group)]
#[tokio::test]
async fn test_var_disabled_routing() {
    let ma = test_utils::setup_modular_agent().await;

    // load var patch
    let var_patch_id =
        test_utils::open_and_start_patch(&ma, "tests/patches/Core_Var_disabled.json")
            .await
            .unwrap();

    // var1 is disabled, but we sent "hello" to it, so the notification should still be sent.
    test_utils::write_and_expect_local_value(&ma, &var_patch_id, "var1", Value::string("hello"))
        .await
        .unwrap();

    // var2 is disabled, so the notification should fail.
    let res = test_utils::expect_local_value(&var_patch_id, "var2", &Value::string("hello")).await;
    assert!(matches!(res, Err(Error::SendMessageFailed(_))));

    ma.quit();
}
