//! Shared spec rebuilding for agents with a numbered family of string configs
//! (`c0`..`c(n-1)` conditions, `k0`..`k(n-1)` keys, ...) sized by an `n` config.

use modular_agent_core::{
    AgentConfigSpec, AgentConfigSpecs, AgentConfigs, AgentError, AgentSpec, AgentValue,
};

const CONFIG_N: &str = "n";
const PORT_DEFAULT: &str = "_";

/// Upper bound for `n`, keeping a stray config value from requesting a huge allocation.
pub(crate) const MAX_N: i64 = 64;

/// Which side of the agent's ports is rebuilt as the numbered `0`..`n-1` set.
pub(crate) enum NumberedPorts {
    /// Inputs become `0`..`n-1` (ZipToObject).
    Inputs,
    /// Outputs become `0`..`n-1` plus the `_` default port (Switch, Match).
    OutputsWithDefault,
}

pub(crate) struct NumberedSpecOptions<'a> {
    /// Prefix of the numbered string configs (`c` for conditions, `k` for keys).
    pub prefix: &'a str,
    /// Static config names carried over into the rebuilt spec, in display order.
    /// Must contain `n`. The other names keep their current value (or their spec
    /// default when unset); callers re-read typed values from `spec.configs`
    /// after the call.
    pub statics: &'a [&'a str],
    /// Default value of `prefix{i}` when the config is absent.
    pub index_default: fn(usize) -> String,
    pub ports: NumberedPorts,
}

/// Regenerates the dynamic part of a numbered-config agent spec: reads `n` (clamped
/// to 1..=MAX_N), rebuilds `configs` / `config_specs` from scratch with the static
/// configs carried over, regenerates the numbered `prefix0`..`prefix(n-1)` string
/// configs, and rewrites the chosen port side to `0`..`n-1`.
///
/// `AgentDefinition::reconcile_spec` moves every config the definition does not
/// declare - which includes the dynamic numbered configs - to a `_`-prefixed key
/// when a patch is loaded, and `AgentData::new` strips those keys afterwards. The
/// numbered lookup therefore falls back to the parked `_`-prefixed key, so saved
/// values survive a reload - which is why `AsAgent::new` implementations must call
/// this on the spec argument before handing it to `AgentData::new`. When called
/// later from `configs_changed`, no `_`-prefixed key exists and the fallback is a
/// no-op.
///
/// Returns `n` and the current values of the numbered configs.
pub(crate) fn update_numbered_spec(
    spec: &mut AgentSpec,
    opts: &NumberedSpecOptions,
) -> Result<(usize, Vec<String>), AgentError> {
    let n = spec
        .configs
        .as_ref()
        .map(|cfg| cfg.get_integer_or(CONFIG_N, 2))
        .unwrap_or(2);
    let n = n.clamp(1, MAX_N) as usize;

    let mut configs = AgentConfigs::new();
    let mut config_specs = AgentConfigSpecs::default();

    for name in opts.statics.iter().copied() {
        let Some(config_spec) = spec
            .config_specs
            .as_ref()
            .and_then(|cs| cs.get(name))
            .cloned()
        else {
            return Err(AgentError::InvalidConfig(format!(
                "config {} must be present",
                name
            )));
        };
        let value = if name == CONFIG_N {
            AgentValue::integer(n as i64)
        } else {
            spec.configs
                .as_ref()
                .and_then(|cfg| cfg.get(name).ok().cloned())
                .unwrap_or_else(|| config_spec.value.clone())
        };
        configs.set(name.to_string(), value);
        config_specs.insert(name.to_string(), config_spec);
    }

    let mut values = Vec::with_capacity(n);
    for i in 0..n {
        let name = format!("{}{}", opts.prefix, i);
        let default = (opts.index_default)(i);
        let v = spec
            .configs
            .as_ref()
            .map(|cfg| {
                if cfg.contains_key(&name) {
                    cfg.get_string_or(&name, default.as_str())
                } else {
                    cfg.get_string_or(&format!("_{}", name), default.as_str())
                }
            })
            .unwrap_or(default);

        values.push(v.clone());

        configs.set(name.clone(), AgentValue::string(v));
        config_specs.insert(
            name,
            AgentConfigSpec {
                value: AgentValue::string_default(),
                type_: Some("string".to_string()),
                ..Default::default()
            },
        );
    }

    spec.configs = Some(configs);
    spec.config_specs = Some(config_specs);

    match opts.ports {
        NumberedPorts::Inputs => {
            spec.inputs = Some((0..n).map(|i| i.to_string()).collect());
        }
        NumberedPorts::OutputsWithDefault => {
            let mut outputs: Vec<String> = (0..n).map(|i| i.to_string()).collect();
            outputs.push(PORT_DEFAULT.to_string());
            spec.outputs = Some(outputs);
        }
    }

    Ok((n, values))
}
