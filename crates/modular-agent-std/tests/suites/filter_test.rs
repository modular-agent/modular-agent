extern crate modular_agent_core as ma;

use im::hashmap;
use ma::{AgentValue, test_utils};

const PATCH: &str = "tests/patches/Std_Filter_test.json";

#[tokio::test]
async fn test_if_number() {
    let ma = test_utils::setup_modular_agent().await;

    let patch_id = test_utils::open_and_start_patch(&ma, PATCH).await.unwrap();

    // cond `> 10`: integer above the bound -> t
    test_utils::write_and_expect_local_value(&ma, &patch_id, "if_in", AgentValue::integer(20))
        .await
        .unwrap();
    test_utils::expect_local_value(&patch_id, "if_t", &AgentValue::integer(20))
        .await
        .unwrap();

    // Integer below the bound -> f
    test_utils::write_and_expect_local_value(&ma, &patch_id, "if_in", AgentValue::integer(5))
        .await
        .unwrap();
    test_utils::expect_local_value(&patch_id, "if_f", &AgentValue::integer(5))
        .await
        .unwrap();

    // Number and Integer are compared alike
    test_utils::write_and_expect_local_value(&ma, &patch_id, "if_in", AgentValue::number(12.5))
        .await
        .unwrap();
    test_utils::expect_local_value(&patch_id, "if_t", &AgentValue::number(12.5))
        .await
        .unwrap();

    ma.quit();
}

#[tokio::test]
async fn test_if_type_mismatch() {
    let ma = test_utils::setup_modular_agent().await;

    let patch_id = test_utils::open_and_start_patch(&ma, PATCH).await.unwrap();

    // A string cannot be ordered against `> 10`, so it is routed to f instead of failing
    test_utils::write_and_expect_local_value(&ma, &patch_id, "if_in", AgentValue::string("hello"))
        .await
        .unwrap();
    test_utils::expect_local_value(&patch_id, "if_f", &AgentValue::string("hello"))
        .await
        .unwrap();

    ma.quit();
}

#[tokio::test]
async fn test_if_string_eq() {
    let ma = test_utils::setup_modular_agent().await;

    let patch_id = test_utils::open_and_start_patch(&ma, PATCH).await.unwrap();

    // Rewrite the condition at runtime through the config port
    test_utils::write_and_expect_local_value(
        &ma,
        &patch_id,
        "if_cond",
        AgentValue::string("== \"abc\""),
    )
    .await
    .unwrap();

    test_utils::write_and_expect_local_value(&ma, &patch_id, "if_in", AgentValue::string("abc"))
        .await
        .unwrap();
    test_utils::expect_local_value(&patch_id, "if_t", &AgentValue::string("abc"))
        .await
        .unwrap();

    test_utils::write_and_expect_local_value(&ma, &patch_id, "if_in", AgentValue::string("xyz"))
        .await
        .unwrap();
    test_utils::expect_local_value(&patch_id, "if_f", &AgentValue::string("xyz"))
        .await
        .unwrap();

    ma.quit();
}

#[tokio::test]
async fn test_if_regex() {
    let ma = test_utils::setup_modular_agent().await;

    let patch_id = test_utils::open_and_start_patch(&ma, PATCH).await.unwrap();

    // A regex literal matches the string value in full
    test_utils::write_and_expect_local_value(
        &ma,
        &patch_id,
        "if_cond",
        AgentValue::string("== /h.*/"),
    )
    .await
    .unwrap();

    test_utils::write_and_expect_local_value(&ma, &patch_id, "if_in", AgentValue::string("hello"))
        .await
        .unwrap();
    test_utils::expect_local_value(&patch_id, "if_t", &AgentValue::string("hello"))
        .await
        .unwrap();

    test_utils::write_and_expect_local_value(&ma, &patch_id, "if_in", AgentValue::string("world"))
        .await
        .unwrap();
    test_utils::expect_local_value(&patch_id, "if_f", &AgentValue::string("world"))
        .await
        .unwrap();

    // `!=` is the exact negation of the same match
    test_utils::write_and_expect_local_value(
        &ma,
        &patch_id,
        "if_cond",
        AgentValue::string("!= /h.*/"),
    )
    .await
    .unwrap();

    test_utils::write_and_expect_local_value(&ma, &patch_id, "if_in", AgentValue::string("hello"))
        .await
        .unwrap();
    test_utils::expect_local_value(&patch_id, "if_f", &AgentValue::string("hello"))
        .await
        .unwrap();

    test_utils::write_and_expect_local_value(&ma, &patch_id, "if_in", AgentValue::string("world"))
        .await
        .unwrap();
    test_utils::expect_local_value(&patch_id, "if_t", &AgentValue::string("world"))
        .await
        .unwrap();

    ma.quit();
}

#[tokio::test]
async fn test_if_bool() {
    let ma = test_utils::setup_modular_agent().await;

    let patch_id = test_utils::open_and_start_patch(&ma, PATCH).await.unwrap();

    test_utils::write_and_expect_local_value(
        &ma,
        &patch_id,
        "if_cond",
        AgentValue::string("!= true"),
    )
    .await
    .unwrap();

    test_utils::write_and_expect_local_value(&ma, &patch_id, "if_in", AgentValue::boolean(false))
        .await
        .unwrap();
    test_utils::expect_local_value(&patch_id, "if_t", &AgentValue::boolean(false))
        .await
        .unwrap();

    test_utils::write_and_expect_local_value(&ma, &patch_id, "if_in", AgentValue::boolean(true))
        .await
        .unwrap();
    test_utils::expect_local_value(&patch_id, "if_f", &AgentValue::boolean(true))
        .await
        .unwrap();

    // `!=` is the exact negation of `==`, so a value of another type matches
    test_utils::write_and_expect_local_value(&ma, &patch_id, "if_in", AgentValue::string("abc"))
        .await
        .unwrap();
    test_utils::expect_local_value(&patch_id, "if_t", &AgentValue::string("abc"))
        .await
        .unwrap();

    ma.quit();
}

#[tokio::test]
async fn test_if_object_key() {
    let ma = test_utils::setup_modular_agent().await;

    let patch_id = test_utils::open_and_start_patch(&ma, PATCH).await.unwrap();

    // The key selects what the condition tests; the whole input is still what gets emitted
    test_utils::write_and_expect_local_value(
        &ma,
        &patch_id,
        "if_cond",
        AgentValue::string("user.age > 18"),
    )
    .await
    .unwrap();

    let adult = AgentValue::object(hashmap! {
        "user".to_string() => AgentValue::object(hashmap! {
            "age".to_string() => AgentValue::integer(20),
        }),
        "name".to_string() => AgentValue::string("a".to_string()),
    });
    test_utils::write_and_expect_local_value(&ma, &patch_id, "if_in", adult.clone())
        .await
        .unwrap();
    test_utils::expect_local_value(&patch_id, "if_t", &adult)
        .await
        .unwrap();

    let child = AgentValue::object(hashmap! {
        "user".to_string() => AgentValue::object(hashmap! {
            "age".to_string() => AgentValue::integer(10),
        }),
        "name".to_string() => AgentValue::string("a".to_string()),
    });
    test_utils::write_and_expect_local_value(&ma, &patch_id, "if_in", child.clone())
        .await
        .unwrap();
    test_utils::expect_local_value(&patch_id, "if_f", &child)
        .await
        .unwrap();

    // A missing field makes the key resolve to null, which is not ordered against 18
    let no_user = AgentValue::object(hashmap! {
        "name".to_string() => AgentValue::string("a".to_string()),
    });
    test_utils::write_and_expect_local_value(&ma, &patch_id, "if_in", no_user.clone())
        .await
        .unwrap();
    test_utils::expect_local_value(&patch_id, "if_f", &no_user)
        .await
        .unwrap();

    // A non-object input cannot resolve the key either, and is routed instead of failing
    test_utils::write_and_expect_local_value(&ma, &patch_id, "if_in", AgentValue::string("hello"))
        .await
        .unwrap();
    test_utils::expect_local_value(&patch_id, "if_f", &AgentValue::string("hello"))
        .await
        .unwrap();

    ma.quit();
}

#[tokio::test]
async fn test_if_object_missing_key_is_null() {
    let ma = test_utils::setup_modular_agent().await;

    let patch_id = test_utils::open_and_start_patch(&ma, PATCH).await.unwrap();

    // `== null` is how a missing field is detected
    test_utils::write_and_expect_local_value(
        &ma,
        &patch_id,
        "if_cond",
        AgentValue::string("user.age == null"),
    )
    .await
    .unwrap();

    let no_user = AgentValue::object(hashmap! {
        "name".to_string() => AgentValue::string("a".to_string()),
    });
    test_utils::write_and_expect_local_value(&ma, &patch_id, "if_in", no_user.clone())
        .await
        .unwrap();
    test_utils::expect_local_value(&patch_id, "if_t", &no_user)
        .await
        .unwrap();

    // A field that is present is not null
    let with_user = AgentValue::object(hashmap! {
        "user".to_string() => AgentValue::object(hashmap! {
            "age".to_string() => AgentValue::integer(20),
        }),
    });
    test_utils::write_and_expect_local_value(&ma, &patch_id, "if_in", with_user.clone())
        .await
        .unwrap();
    test_utils::expect_local_value(&patch_id, "if_f", &with_user)
        .await
        .unwrap();

    ma.quit();
}

#[tokio::test]
async fn test_if_invalid_cond() {
    let ma = test_utils::setup_modular_agent().await;

    let patch_id = test_utils::open_and_start_patch(&ma, PATCH).await.unwrap();

    // An invalid condition set at runtime discards the previous one, so processing fails
    // instead of routing by the condition the config no longer holds
    test_utils::write_and_expect_local_value(
        &ma,
        &patch_id,
        "if_cond",
        AgentValue::string("> abc"),
    )
    .await
    .unwrap();

    test_utils::write_and_expect_local_value(&ma, &patch_id, "if_in", AgentValue::integer(20))
        .await
        .unwrap();
    // Nothing is emitted at all - not on `t`, and not on `f` either
    assert!(
        test_utils::recv_external_output_with_timeout(test_utils::DEFAULT_OUTPUT_TIMEOUT)
            .await
            .is_err()
    );

    ma.quit();
}

#[tokio::test]
async fn test_switch() {
    let ma = test_utils::setup_modular_agent().await;

    let patch_id = test_utils::open_and_start_patch(&ma, PATCH).await.unwrap();

    // c0 `> 10`, c1 `> 5`: both match, but the first one wins
    test_utils::write_and_expect_local_value(&ma, &patch_id, "switch_in", AgentValue::integer(20))
        .await
        .unwrap();
    test_utils::expect_local_value(&patch_id, "switch_0", &AgentValue::integer(20))
        .await
        .unwrap();

    // Only c1 matches
    test_utils::write_and_expect_local_value(&ma, &patch_id, "switch_in", AgentValue::integer(7))
        .await
        .unwrap();
    test_utils::expect_local_value(&patch_id, "switch_1", &AgentValue::integer(7))
        .await
        .unwrap();

    // No condition matches
    test_utils::write_and_expect_local_value(&ma, &patch_id, "switch_in", AgentValue::integer(3))
        .await
        .unwrap();
    test_utils::expect_local_value(&patch_id, "switch_default", &AgentValue::integer(3))
        .await
        .unwrap();

    // An incomparable value matches no condition either
    test_utils::write_and_expect_local_value(
        &ma,
        &patch_id,
        "switch_in",
        AgentValue::string("hello"),
    )
    .await
    .unwrap();
    test_utils::expect_local_value(&patch_id, "switch_default", &AgentValue::string("hello"))
        .await
        .unwrap();

    ma.quit();
}

#[tokio::test]
async fn test_switch_config_update() {
    let ma = test_utils::setup_modular_agent().await;

    let patch_id = test_utils::open_and_start_patch(&ma, PATCH).await.unwrap();

    // Raising n exposes the new port `2` and the matching c2 config
    test_utils::write_and_expect_local_value(&ma, &patch_id, "switch_n", AgentValue::integer(3))
        .await
        .unwrap();
    test_utils::write_and_expect_local_value(
        &ma,
        &patch_id,
        "switch_c2",
        AgentValue::string("== 3"),
    )
    .await
    .unwrap();

    test_utils::write_and_expect_local_value(&ma, &patch_id, "switch_in", AgentValue::integer(3))
        .await
        .unwrap();
    test_utils::expect_local_value(&patch_id, "switch_2", &AgentValue::integer(3))
        .await
        .unwrap();

    // An invalid condition set at runtime never matches instead of keeping the old one
    test_utils::write_and_expect_local_value(
        &ma,
        &patch_id,
        "switch_c0",
        AgentValue::string("> abc"),
    )
    .await
    .unwrap();

    test_utils::write_and_expect_local_value(&ma, &patch_id, "switch_in", AgentValue::integer(20))
        .await
        .unwrap();
    test_utils::expect_local_value(&patch_id, "switch_1", &AgentValue::integer(20))
        .await
        .unwrap();

    // Lowering n again drops the extra conditions along with their ports
    test_utils::write_and_expect_local_value(&ma, &patch_id, "switch_n", AgentValue::integer(2))
        .await
        .unwrap();

    test_utils::write_and_expect_local_value(&ma, &patch_id, "switch_in", AgentValue::integer(3))
        .await
        .unwrap();
    test_utils::expect_local_value(&patch_id, "switch_default", &AgentValue::integer(3))
        .await
        .unwrap();

    ma.quit();
}

#[tokio::test]
async fn test_switch_regex() {
    let ma = test_utils::setup_modular_agent().await;

    let patch_id = test_utils::open_and_start_patch(&ma, PATCH).await.unwrap();

    // c0 becomes a regex on the `status` field. c1 stays `> 5` from the patch: with
    // no key it tests the input object itself, which has no numeric value, so it never
    // matches here - a non-matching status therefore routes to default.
    test_utils::write_and_expect_local_value(
        &ma,
        &patch_id,
        "switch_c0",
        AgentValue::string("status == /err.*/"),
    )
    .await
    .unwrap();

    // c0 matches in full, and the whole input object is what gets emitted
    let failed = AgentValue::object(hashmap! {
        "status".to_string() => AgentValue::string("error".to_string()),
    });
    test_utils::write_and_expect_local_value(&ma, &patch_id, "switch_in", failed.clone())
        .await
        .unwrap();
    test_utils::expect_local_value(&patch_id, "switch_0", &failed)
        .await
        .unwrap();

    // A status that only partly matches the anchored pattern does not match
    let partial = AgentValue::object(hashmap! {
        "status".to_string() => AgentValue::string("my error".to_string()),
    });
    test_utils::write_and_expect_local_value(&ma, &patch_id, "switch_in", partial.clone())
        .await
        .unwrap();
    test_utils::expect_local_value(&patch_id, "switch_default", &partial)
        .await
        .unwrap();

    ma.quit();
}

#[tokio::test]
async fn test_switch_object_key() {
    let ma = test_utils::setup_modular_agent().await;

    let patch_id = test_utils::open_and_start_patch(&ma, PATCH).await.unwrap();

    // Each condition carries its own key, so they can look at different fields.
    // c1 stays `> 5` from the patch: with no key it tests the input object itself,
    // and an object has no numeric value, so it never matches here.
    test_utils::write_and_expect_local_value(&ma, &patch_id, "switch_n", AgentValue::integer(3))
        .await
        .unwrap();
    test_utils::write_and_expect_local_value(
        &ma,
        &patch_id,
        "switch_c0",
        AgentValue::string("status == \"error\""),
    )
    .await
    .unwrap();
    test_utils::write_and_expect_local_value(
        &ma,
        &patch_id,
        "switch_c2",
        AgentValue::string("retry > 3"),
    )
    .await
    .unwrap();

    // c0 matches, and the whole input object is what gets emitted
    let failed = AgentValue::object(hashmap! {
        "status".to_string() => AgentValue::string("error".to_string()),
        "retry".to_string() => AgentValue::integer(0),
    });
    test_utils::write_and_expect_local_value(&ma, &patch_id, "switch_in", failed.clone())
        .await
        .unwrap();
    test_utils::expect_local_value(&patch_id, "switch_0", &failed)
        .await
        .unwrap();

    // Only c2 matches, on a different field than c0
    let retried = AgentValue::object(hashmap! {
        "status".to_string() => AgentValue::string("ok".to_string()),
        "retry".to_string() => AgentValue::integer(5),
    });
    test_utils::write_and_expect_local_value(&ma, &patch_id, "switch_in", retried.clone())
        .await
        .unwrap();
    test_utils::expect_local_value(&patch_id, "switch_2", &retried)
        .await
        .unwrap();

    // No condition matches
    let plain = AgentValue::object(hashmap! {
        "status".to_string() => AgentValue::string("ok".to_string()),
        "retry".to_string() => AgentValue::integer(0),
    });
    test_utils::write_and_expect_local_value(&ma, &patch_id, "switch_in", plain.clone())
        .await
        .unwrap();
    test_utils::expect_local_value(&patch_id, "switch_default", &plain)
        .await
        .unwrap();

    ma.quit();
}

#[tokio::test]
async fn test_match() {
    let ma = test_utils::setup_modular_agent().await;

    let patch_id = test_utils::open_and_start_patch(&ma, PATCH).await.unwrap();

    // The patch leaves `key` empty, so the input value itself is compared against
    // c0 `"a"` and c1 `/b.*/`
    test_utils::write_and_expect_local_value(&ma, &patch_id, "match_in", AgentValue::string("a"))
        .await
        .unwrap();
    test_utils::expect_local_value(&patch_id, "match_0", &AgentValue::string("a"))
        .await
        .unwrap();

    // The regex case matches in full
    test_utils::write_and_expect_local_value(&ma, &patch_id, "match_in", AgentValue::string("bcd"))
        .await
        .unwrap();
    test_utils::expect_local_value(&patch_id, "match_1", &AgentValue::string("bcd"))
        .await
        .unwrap();

    // No case matches
    test_utils::write_and_expect_local_value(&ma, &patch_id, "match_in", AgentValue::string("c"))
        .await
        .unwrap();
    test_utils::expect_local_value(&patch_id, "match_default", &AgentValue::string("c"))
        .await
        .unwrap();

    // Comparison is by type as well as value: the case `10` matches the number, not the
    // string "10"
    test_utils::write_and_expect_local_value(&ma, &patch_id, "match_c0", AgentValue::string("10"))
        .await
        .unwrap();

    test_utils::write_and_expect_local_value(&ma, &patch_id, "match_in", AgentValue::integer(10))
        .await
        .unwrap();
    test_utils::expect_local_value(&patch_id, "match_0", &AgentValue::integer(10))
        .await
        .unwrap();

    test_utils::write_and_expect_local_value(&ma, &patch_id, "match_in", AgentValue::string("10"))
        .await
        .unwrap();
    test_utils::expect_local_value(&patch_id, "match_default", &AgentValue::string("10"))
        .await
        .unwrap();

    ma.quit();
}

#[tokio::test]
async fn test_match_key() {
    let ma = test_utils::setup_modular_agent().await;

    let patch_id = test_utils::open_and_start_patch(&ma, PATCH).await.unwrap();

    // The key selects what is compared; c1 becomes `null` so that a missing key is caught
    test_utils::write_and_expect_local_value(
        &ma,
        &patch_id,
        "match_key",
        AgentValue::string("user.status"),
    )
    .await
    .unwrap();
    test_utils::write_and_expect_local_value(
        &ma,
        &patch_id,
        "match_c0",
        AgentValue::string("\"error\""),
    )
    .await
    .unwrap();
    test_utils::write_and_expect_local_value(
        &ma,
        &patch_id,
        "match_c1",
        AgentValue::string("null"),
    )
    .await
    .unwrap();

    // c0 matches, and the whole input object is what gets emitted
    let failed = AgentValue::object(hashmap! {
        "user".to_string() => AgentValue::object(hashmap! {
            "status".to_string() => AgentValue::string("error".to_string()),
        }),
        "name".to_string() => AgentValue::string("a".to_string()),
    });
    test_utils::write_and_expect_local_value(&ma, &patch_id, "match_in", failed.clone())
        .await
        .unwrap();
    test_utils::expect_local_value(&patch_id, "match_0", &failed)
        .await
        .unwrap();

    // A key that does not resolve is compared as null, so c1 catches it
    let missing = AgentValue::object(hashmap! {
        "name".to_string() => AgentValue::string("a".to_string()),
    });
    test_utils::write_and_expect_local_value(&ma, &patch_id, "match_in", missing.clone())
        .await
        .unwrap();
    test_utils::expect_local_value(&patch_id, "match_1", &missing)
        .await
        .unwrap();

    // So is a scalar input, which is not an object at all
    test_utils::write_and_expect_local_value(&ma, &patch_id, "match_in", AgentValue::integer(20))
        .await
        .unwrap();
    test_utils::expect_local_value(&patch_id, "match_1", &AgentValue::integer(20))
        .await
        .unwrap();

    // A resolved key that equals no case goes to default
    let ok = AgentValue::object(hashmap! {
        "user".to_string() => AgentValue::object(hashmap! {
            "status".to_string() => AgentValue::string("ok".to_string()),
        }),
    });
    test_utils::write_and_expect_local_value(&ma, &patch_id, "match_in", ok.clone())
        .await
        .unwrap();
    test_utils::expect_local_value(&patch_id, "match_default", &ok)
        .await
        .unwrap();

    ma.quit();
}

#[tokio::test]
async fn test_match_config_update() {
    let ma = test_utils::setup_modular_agent().await;

    let patch_id = test_utils::open_and_start_patch(&ma, PATCH).await.unwrap();

    // Raising n exposes the new port `2` and the matching c2 config
    test_utils::write_and_expect_local_value(&ma, &patch_id, "match_n", AgentValue::integer(3))
        .await
        .unwrap();
    test_utils::write_and_expect_local_value(&ma, &patch_id, "match_c2", AgentValue::string("3"))
        .await
        .unwrap();

    test_utils::write_and_expect_local_value(&ma, &patch_id, "match_in", AgentValue::integer(3))
        .await
        .unwrap();
    test_utils::expect_local_value(&patch_id, "match_2", &AgentValue::integer(3))
        .await
        .unwrap();

    // An invalid case set at runtime never matches instead of keeping the old one
    test_utils::write_and_expect_local_value(&ma, &patch_id, "match_c0", AgentValue::string("abc"))
        .await
        .unwrap();

    test_utils::write_and_expect_local_value(&ma, &patch_id, "match_in", AgentValue::string("a"))
        .await
        .unwrap();
    test_utils::expect_local_value(&patch_id, "match_default", &AgentValue::string("a"))
        .await
        .unwrap();

    // Lowering n again drops the extra cases along with their ports
    test_utils::write_and_expect_local_value(&ma, &patch_id, "match_n", AgentValue::integer(2))
        .await
        .unwrap();

    test_utils::write_and_expect_local_value(&ma, &patch_id, "match_in", AgentValue::integer(3))
        .await
        .unwrap();
    test_utils::expect_local_value(&patch_id, "match_default", &AgentValue::integer(3))
        .await
        .unwrap();

    ma.quit();
}
