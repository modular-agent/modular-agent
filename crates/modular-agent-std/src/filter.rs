use std::cmp::Ordering;

use modular_agent_core::{
    AgentConfigSpec, AgentConfigSpecs, AgentConfigs, AgentContext, AgentData, AgentError,
    AgentOutput, AgentSpec, AgentValue, AsAgent, ModularAgent, async_trait, modular_agent,
};
use regex::Regex;

use crate::data::get_nested_value;

const CATEGORY: &str = "Std/Filter";

const PORT_INPUT: &str = "input";
const PORT_T: &str = "t";
const PORT_F: &str = "f";
const PORT_DEFAULT: &str = "_";

const CONFIG_COND: &str = "cond";
const CONFIG_N: &str = "n";
const CONFIG_KEY: &str = "key";
const CONFIG_C1: &str = "c1";
const CONFIG_C2: &str = "c2";

/// Upper bound for the Switch / Match agents' `n` config.
const MAX_N: i64 = 64;

// Condition expression: `[<path>] <operator> <JSON literal>`

#[derive(Clone, Copy, Debug, PartialEq)]
enum CondOp {
    Eq,
    Ne,
    Gt,
    Ge,
    Lt,
    Le,
}

impl CondOp {
    /// Order comparisons (`>`, `>=`, `<`, `<=`) are only meaningful for numbers and strings.
    fn is_order(&self) -> bool {
        matches!(self, CondOp::Gt | CondOp::Ge | CondOp::Lt | CondOp::Le)
    }
}

/// A compiled regex literal. `regex::Regex` does not implement `PartialEq`, which `CondLit`
/// needs for the derived equality used in unit tests, so it is wrapped here and compared by
/// its source pattern.
#[derive(Clone, Debug)]
struct CondRegex(Regex);

impl PartialEq for CondRegex {
    fn eq(&self, other: &Self) -> bool {
        self.0.as_str() == other.0.as_str()
    }
}

#[derive(Clone, Debug, PartialEq)]
enum CondLit {
    /// Kept apart from `Number` so that integers beyond f64's exact range compare exactly.
    Integer(i64),
    Number(f64),
    String(String),
    Boolean(bool),
    Null,
    /// A regular expression matched against the string value in full (anchored `^(?:...)$`).
    Regex(CondRegex),
}

#[derive(Clone, Debug, PartialEq)]
struct Cond {
    /// Path to the value to test, empty when the input value itself is tested.
    path: Vec<String>,
    op: CondOp,
    lit: CondLit,
}

/// Parses a condition expression such as `> 10`, `== "abc"` or `user.age >= 18`.
///
/// The part before the first operator character is the path; scanning stops there, so an
/// operator character inside the literal (`name == "a>b"`) is never mistaken for the
/// operator. Key names containing `=`, `!`, `<` or `>` are therefore not addressable.
///
/// Two-character operators are matched before one-character ones so that `>=` is not
/// read as `>` followed by the literal `= 10`.
fn parse_cond(src: &str) -> Result<Cond, AgentError> {
    let src = src.trim();

    let Some(op_at) = src.find(['=', '!', '<', '>']) else {
        return Err(AgentError::InvalidConfig(format!(
            "Condition must be `[path] <operator> <literal>`, for example `> 10` or `user.age >= 18`: {}",
            src
        )));
    };
    let (path_src, op_src) = src.split_at(op_at);

    let path = parse_path(path_src)?;

    let (op, rest) = if let Some(rest) = op_src.strip_prefix("==") {
        (CondOp::Eq, rest)
    } else if let Some(rest) = op_src.strip_prefix("!=") {
        (CondOp::Ne, rest)
    } else if let Some(rest) = op_src.strip_prefix(">=") {
        (CondOp::Ge, rest)
    } else if let Some(rest) = op_src.strip_prefix("<=") {
        (CondOp::Le, rest)
    } else if let Some(rest) = op_src.strip_prefix('>') {
        (CondOp::Gt, rest)
    } else if let Some(rest) = op_src.strip_prefix('<') {
        (CondOp::Lt, rest)
    } else {
        return Err(AgentError::InvalidConfig(format!(
            "Condition must start with one of ==, !=, >, >=, <, <=: {}",
            src
        )));
    };

    let rest = rest.trim();
    if rest.is_empty() {
        return Err(AgentError::InvalidConfig(format!(
            "Condition is missing a literal: {}",
            src
        )));
    }

    let lit = parse_lit(rest)?;

    if op.is_order() && matches!(lit, CondLit::Boolean(_) | CondLit::Null | CondLit::Regex(_)) {
        return Err(AgentError::InvalidConfig(format!(
            "Order comparison is not supported for boolean, null or regex literals: {}",
            src
        )));
    }

    Ok(Cond { path, op, lit })
}

/// Parses a dot-separated path such as `user.age` into its segments. A blank path yields an
/// empty vector, which addresses the input value itself.
fn parse_path(src: &str) -> Result<Vec<String>, AgentError> {
    let src = src.trim();
    if src.is_empty() {
        return Ok(Vec::new());
    }

    let mut path = Vec::new();
    for segment in src.split('.') {
        let segment = segment.trim();
        if segment.is_empty() {
            return Err(AgentError::InvalidConfig(format!(
                "Path has an empty segment: {}",
                src
            )));
        }
        path.push(segment.to_string());
    }
    Ok(path)
}

/// Parses a literal: a regex between slashes, or any scalar JSON value.
fn parse_lit(src: &str) -> Result<CondLit, AgentError> {
    let src = src.trim();
    if src.is_empty() {
        return Err(AgentError::InvalidConfig("Literal is missing".into()));
    }

    // A regex literal is `/pattern/`. It is handled before JSON parsing so that a leading
    // `/` is never read as an (invalid) JSON value. The pattern is everything between the
    // first and last `/`; requiring the closing `/` to be the last character means a `/`
    // inside the pattern needs no escaping (`/a/b/` is the pattern `a/b`).
    let Some(after_open) = src.strip_prefix('/') else {
        let json: serde_json::Value = serde_json::from_str(src)
            .map_err(|e| AgentError::InvalidConfig(format!("Invalid literal `{}`: {}", src, e)))?;

        return match json {
            serde_json::Value::Null => Ok(CondLit::Null),
            serde_json::Value::Bool(b) => Ok(CondLit::Boolean(b)),
            serde_json::Value::Number(n) => {
                if let Some(i) = n.as_i64() {
                    Ok(CondLit::Integer(i))
                } else if let Some(f) = n.as_f64() {
                    Ok(CondLit::Number(f))
                } else {
                    Err(AgentError::InvalidConfig(format!(
                        "Literal is not representable as a number: {}",
                        src
                    )))
                }
            }
            serde_json::Value::String(s) => Ok(CondLit::String(s)),
            _ => Err(AgentError::InvalidConfig(format!(
                "Array and object literals are not supported: {}",
                src
            ))),
        };
    };

    let Some(pattern) = after_open.strip_suffix('/') else {
        return Err(AgentError::InvalidConfig(format!(
            "Regex literal must be closed with a `/`: {}",
            src
        )));
    };
    // Validate the bare pattern first: an unbalanced pattern such as `a)|(b` is invalid
    // on its own but would merge with the anchoring wrapper below into a valid -
    // and unanchored - regex instead of being reported as an error.
    Regex::new(pattern).map_err(|e| {
        AgentError::InvalidConfig(format!("Invalid regex literal `/{}/`: {}", pattern, e))
    })?;
    // Anchor the whole match. `(?:...)` groups the pattern so an alternation such as
    // `a|b` is anchored as a whole rather than as `^a` or `b$`.
    let re = Regex::new(&format!("^(?:{pattern})$")).map_err(|e| {
        AgentError::InvalidConfig(format!("Invalid regex literal `/{}/`: {}", pattern, e))
    })?;
    Ok(CondLit::Regex(CondRegex(re)))
}

/// Parses a condition config value. An empty or blank config yields `None`.
fn load_cond(src: &str) -> Result<Option<Cond>, AgentError> {
    if src.trim().is_empty() {
        return Ok(None);
    }
    parse_cond(src).map(Some)
}

/// Parses a literal config value. An empty or blank config yields `None`.
fn load_lit(src: &str) -> Result<Option<CondLit>, AgentError> {
    if src.trim().is_empty() {
        return Ok(None);
    }
    parse_lit(src).map(Some)
}

/// Equality between an input value and a literal. A type mismatch is simply `false`;
/// no implicit coercion (the string `"10"` does not equal the number `10`).
fn eq_cond(lit: &CondLit, value: &AgentValue) -> bool {
    match lit {
        CondLit::Integer(i) => match value {
            AgentValue::Integer(v) => v == i,
            _ => value.as_f64() == Some(*i as f64),
        },
        CondLit::Number(n) => value.as_f64() == Some(*n),
        CondLit::String(s) => value.as_str() == Some(s.as_str()),
        CondLit::Boolean(b) => value.as_bool() == Some(*b),
        CondLit::Null => value.is_unit(),
        // A non-string value never matches, so `==` is false and `!=` (its negation) is true.
        CondLit::Regex(re) => value.as_str().is_some_and(|s| re.0.is_match(s)),
    }
}

/// Orders an input value against a literal, or `None` when they are not comparable.
fn cmp_cond(lit: &CondLit, value: &AgentValue) -> Option<Ordering> {
    match lit {
        CondLit::Integer(i) => match value {
            AgentValue::Integer(v) => Some(v.cmp(i)),
            _ => value.as_f64().and_then(|v| v.partial_cmp(&(*i as f64))),
        },
        CondLit::Number(n) => value.as_f64().and_then(|v| v.partial_cmp(n)),
        CondLit::String(s) => value.as_str().map(|v| v.cmp(s.as_str())),
        // Order comparisons against these are rejected at parse time; unreachable in practice.
        CondLit::Boolean(_) | CondLit::Null | CondLit::Regex(_) => None,
    }
}

/// Evaluates a condition against an input value.
///
/// When the condition carries a path, the value at that path is tested; a path that does
/// not resolve is tested as null instead of raising an error, so `== null` detects a
/// missing field.
///
/// `!=` is the exact negation of `==`, so a value of a different type than the literal
/// matches. Order comparisons against an incomparable value yield `false` instead of an
/// error, so that a type mismatch routes the value instead of stopping the flow.
fn eval_cond(cond: &Cond, value: &AgentValue) -> bool {
    // `AgentValue` has drop glue, so a temporary cannot be promoted to a `'static`
    // reference; bind it to a local that outlives `target`.
    let unit = AgentValue::unit();
    let target = if cond.path.is_empty() {
        value
    } else {
        get_nested_value(value, &cond.path).unwrap_or(&unit)
    };

    match cond.op {
        CondOp::Eq => eq_cond(&cond.lit, target),
        CondOp::Ne => !eq_cond(&cond.lit, target),
        _ => {
            let Some(ord) = cmp_cond(&cond.lit, target) else {
                return false;
            };
            match cond.op {
                CondOp::Gt => ord == Ordering::Greater,
                CondOp::Ge => ord != Ordering::Less,
                CondOp::Lt => ord == Ordering::Less,
                CondOp::Le => ord != Ordering::Greater,
                CondOp::Eq | CondOp::Ne => unreachable!(),
            }
        }
    }
}

/// Routes the input value to `t` or `f` depending on a condition.
///
/// The condition is written as `[path] <operator> <literal>`, for example `> 10`,
/// `== "abc"` or `user.age >= 18`. Supported operators are `==`, `!=`, `>`, `>=`, `<` and
/// `<=`; supported literals are numbers, strings, booleans, `null` and regular expressions.
/// Order operators reject boolean, null and regex literals at parse time.
///
/// A regex literal is written between slashes, `== /err.*/`, and is matched against a string
/// value in full: the pattern is implicitly anchored as `^(?:...)$`, so `/err.*/` matches
/// `"error"` but not `"my error"` - write `/.*err.*/` for a substring match. Use the inline
/// `(?i)` flag for a case-insensitive match, as in `/(?i)error/`. A non-string value (a
/// number, an object, or an unresolved path) never matches, so `==` is false and `!=` is
/// true for it. A `/` inside the pattern needs no escaping as long as the literal still ends
/// with a `/` (`/a/b/` is the pattern `a/b`). An invalid regex is a configuration error,
/// reported the same way as any other invalid condition.
///
/// The path is a dot-separated list of object keys. When it is omitted, the input value
/// itself is tested. Whether or not a path is used, the value emitted on `t` / `f` is
/// always the original input value - the path only selects what the condition looks at.
/// A path that does not resolve (the input is not an object, a key is missing, or an
/// intermediate value is not an object) is tested as `null` rather than raising an error,
/// so `user.age == null` detects a missing field. Key names containing `=`, `!`, `<` or
/// `>` cannot be addressed, because the first such character ends the path.
///
/// Numbers compare through their numeric value, so `== 10` matches both an integer and a
/// float input, but never the string `"10"`. Values that cannot be compared with the
/// literal (for example a string input against `> 10`) are routed to `f` rather than
/// raising an error, so the flow keeps running. `!=` is the exact negation of `==`, so a
/// value of a different type than the literal is a match.
///
/// # Ports
/// - Input `input`: Value to test.
/// - Output `t`: The input value, when the condition matches.
/// - Output `f`: The input value, when the condition does not match.
///
/// # Configuration
/// - `cond`: Condition expression. Processing fails while it is empty or invalid.
///
/// # Example
/// With `cond` set to `user.age > 18`, the input `{"user": {"age": 20}, "name": "a"}` is
/// emitted unchanged on `t`, while `{"user": {"age": 10}}` and `{"name": "a"}` are
/// emitted unchanged on `f`.
#[modular_agent(
    title = "If",
    category = CATEGORY,
    inputs = [PORT_INPUT],
    outputs = [PORT_T, PORT_F],
    string_config(name = CONFIG_COND),
    hint(color=5),
)]
struct IfAgent {
    data: AgentData,
    cond: Option<Cond>,
}

impl IfAgent {
    fn load_cond_config(spec: &AgentSpec) -> Result<Option<Cond>, AgentError> {
        let src = spec
            .configs
            .as_ref()
            .map(|cfg| cfg.get_string_or_default(CONFIG_COND))
            .unwrap_or_default();
        load_cond(&src)
    }
}

#[async_trait]
impl AsAgent for IfAgent {
    fn new(ma: ModularAgent, id: String, spec: AgentSpec) -> Result<Self, AgentError> {
        // Keep an invalid condition from blocking the load of a preset; it is reported
        // on the first process() call instead.
        let cond = Self::load_cond_config(&spec).unwrap_or(None);
        Ok(Self {
            data: AgentData::new(ma, id, spec),
            cond,
        })
    }

    fn configs_changed(&mut self) -> Result<(), AgentError> {
        // The config value is already committed when this is called, so the previous
        // condition must be dropped even when the new one fails to parse. Otherwise the
        // agent would keep routing by a condition the config no longer holds.
        match Self::load_cond_config(&self.data.spec) {
            Ok(cond) => {
                self.cond = cond;
                Ok(())
            }
            Err(e) => {
                self.cond = None;
                Err(e)
            }
        }
    }

    async fn process(
        &mut self,
        ctx: AgentContext,
        _port: String,
        value: AgentValue,
    ) -> Result<(), AgentError> {
        let Some(cond) = self.cond.as_ref() else {
            return Err(AgentError::InvalidConfig(
                "config cond must be a valid condition".into(),
            ));
        };
        if eval_cond(cond, &value) {
            self.output(ctx, PORT_T, value).await
        } else {
            self.output(ctx, PORT_F, value).await
        }
    }
}

/// Regenerates the dynamic part of a Switch / Match spec: reads `n` (clamped to 1..=64),
/// carries over the `n` config and the given extra string configs, and rebuilds the
/// numbered `c1`..`cn` configs and the output ports `0`..`n-1` plus `_`.
///
/// Returns `n`, the current values of the extra configs (in the given order) and the
/// current values of `c1`..`cn`.
fn update_numbered_spec(
    spec: &mut AgentSpec,
    extra_strings: &[&str],
) -> Result<(usize, Vec<String>, Vec<String>), AgentError> {
    let n = spec
        .configs
        .as_ref()
        .map(|cfg| cfg.get_integer_or(CONFIG_N, 2))
        .unwrap_or(2);
    // The upper bound keeps a stray config value from requesting a huge allocation.
    let n = n.clamp(1, MAX_N) as usize;

    let extra_values: Vec<String> = extra_strings
        .iter()
        .map(|name| {
            spec.configs
                .as_ref()
                .map(|cfg| cfg.get_string_or_default(name))
                .unwrap_or_default()
        })
        .collect();

    // Dynamic generation of config definitions (ConfigSpecs)
    let mut configs = AgentConfigs::new();
    let mut config_specs = AgentConfigSpecs::default();

    // Re-set required configurations
    for name in extra_strings.iter().copied().chain([CONFIG_N]) {
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
        config_specs.insert(name.to_string(), config_spec);
    }
    for (name, value) in extra_strings.iter().zip(&extra_values) {
        configs.set(name.to_string(), AgentValue::string(value.clone()));
    }
    configs.set(CONFIG_N.to_string(), AgentValue::integer(n as i64));

    let mut srcs = Vec::with_capacity(n);
    for i in 1..=n {
        let name = format!("c{}", i);
        // `AgentDefinition::reconcile_spec` moves every config the definition does not
        // declare - which includes the dynamic `c1`..`cn` - to a `_`-prefixed key when a
        // preset is loaded. Fall back to it so saved values survive a reload.
        let v = spec
            .configs
            .as_ref()
            .map(|cfg| {
                if cfg.contains_key(&name) {
                    cfg.get_string_or(&name, "")
                } else {
                    cfg.get_string_or(&format!("_{}", name), "")
                }
            })
            .unwrap_or_default();

        srcs.push(v.clone());

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

    let mut outputs: Vec<String> = (0..n).map(|i| i.to_string()).collect();
    outputs.push(PORT_DEFAULT.to_string());
    spec.outputs = Some(outputs);

    Ok((n, extra_values, srcs))
}

/// Parses each source with `parse`, keeping a failed one as a never-matching `None`, and
/// returns the first error alongside the results.
fn parse_all<T>(
    srcs: &[String],
    parse: impl Fn(&str) -> Result<Option<T>, AgentError>,
) -> (Vec<Option<T>>, Option<AgentError>) {
    let mut first_error = None;
    let mut parsed = Vec::with_capacity(srcs.len());
    for src in srcs {
        match parse(src) {
            Ok(v) => parsed.push(v),
            Err(e) => {
                parsed.push(None);
                first_error.get_or_insert(e);
            }
        }
    }
    (parsed, first_error)
}

/// Routes the input value to the first of n conditions that matches.
///
/// The `n` config controls how many `c1`..`cn` conditions exist, and how many
/// numbered output ports `0`..`n-1` are exposed. Conditions are evaluated in order and
/// the value is emitted on the port matching the first successful condition; when none
/// matches, it is emitted on `_`. An empty condition never matches, and so does an
/// invalid one - a condition set at runtime is additionally reported as a configuration
/// error, while an invalid condition loaded from a preset is only kept as never-matching.
///
/// Condition syntax and comparison semantics are the same as the If agent:
/// `[path] <operator> <literal>` with `==`, `!=`, `>`, `>=`, `<`, `<=` and number, string,
/// boolean, null or regex literals; order operators reject boolean, null and regex literals.
/// A regex literal such as `== /err.*/` matches a string
/// value in full (implicitly anchored as `^(?:...)$`; use `(?i)` for case-insensitivity), and
/// a non-string value never matches it. Values that cannot be compared with a literal do not
/// match that condition instead of raising an error, and an invalid regex is kept as a
/// never-matching condition like any other invalid one.
///
/// Each condition carries its own optional path, so different conditions can look at
/// different fields of the same input. The condition tests the value at the path, but the
/// value emitted is always the original input value. A path that does not resolve is
/// tested as `null`, so `user.age == null` detects a missing field. Key names containing
/// `=`, `!`, `<` or `>` cannot be addressed, because the first such character ends the path.
///
/// # Ports
/// - Input `input`: Value to test.
/// - Output `0`..`n-1`: The input value, emitted on the port of the first matching condition.
/// - Output `_`: The input value, when no condition matches.
///
/// # Configuration
/// - `n`: Number of conditions and numbered output ports, clamped to 1..=64 (default: 2)
/// - `c1`..`cn`: Condition expressions, evaluated in order.
///
/// # Example
/// With `n` = 2, `c1` = `status == "error"` and `c2` = `retry > 3`, the input
/// `{"status": "error", "retry": 0}` is emitted unchanged on `0`, `{"status": "ok",
/// "retry": 5}` on `1`, and `{"status": "ok", "retry": 0}` on `_`.
#[modular_agent(
    title = "Switch",
    category = CATEGORY,
    inputs = [PORT_INPUT],
    outputs = ["0", "1", PORT_DEFAULT],
    integer_config(name = CONFIG_N, default = 2),
    // `c1`..`c2` match the default `n`, so a freshly placed agent already exposes them;
    // `update_numbered_spec` takes over once `n` changes.
    string_config(name = CONFIG_C1),
    string_config(name = CONFIG_C2),
    hint(color=5, height=2),
)]
struct SwitchAgent {
    data: AgentData,
    n: usize,

    // Raw condition strings, kept to detect config changes
    cond_srcs: Vec<String>,

    // Optimization: Pre-parsed conditions
    conds: Vec<Option<Cond>>,
}

#[async_trait]
impl AsAgent for SwitchAgent {
    fn new(ma: ModularAgent, id: String, mut spec: AgentSpec) -> Result<Self, AgentError> {
        let (n, _, cond_srcs) = update_numbered_spec(&mut spec, &[])?;
        // Invalid conditions are kept as never-matching instead of blocking the load.
        let (conds, _) = parse_all(&cond_srcs, load_cond);
        let data = AgentData::new(ma, id, spec);
        Ok(Self {
            data,
            n,
            cond_srcs,
            conds,
        })
    }

    fn configs_changed(&mut self) -> Result<(), AgentError> {
        let (n, _, cond_srcs) = update_numbered_spec(&mut self.data.spec, &[])?;
        if n == self.n && cond_srcs == self.cond_srcs {
            return Ok(());
        }

        // `update_numbered_spec` has already rewritten the spec, so the parsed state has
        // to be committed as a whole even when a condition is invalid; otherwise `conds`
        // could outlive the output ports it routes to. An invalid condition is kept as
        // never-matching, like at load time, and the parse error is reported afterwards.
        let (conds, first_error) = parse_all(&cond_srcs, load_cond);

        self.n = n;
        self.cond_srcs = cond_srcs;
        self.conds = conds;
        self.emit_agent_spec_updated();

        match first_error {
            Some(e) => Err(e),
            None => Ok(()),
        }
    }

    async fn process(
        &mut self,
        ctx: AgentContext,
        _port: String,
        value: AgentValue,
    ) -> Result<(), AgentError> {
        let matched = self
            .conds
            .iter()
            .position(|cond| cond.as_ref().is_some_and(|c| eval_cond(c, &value)));

        match matched {
            Some(i) => self.output(ctx, i.to_string(), value).await,
            None => self.output(ctx, PORT_DEFAULT, value).await,
        }
    }
}

/// Routes the input value to the first of n case values it is equal to.
///
/// A single `key` selects what to compare, and each `c1`..`cn` holds one candidate value;
/// there is no operator, the comparison is always equality. Use the Switch agent instead
/// when the branches need different operators or different keys.
///
/// The `n` config controls how many `c1`..`cn` case values exist, and how many numbered
/// output ports `0`..`n-1` are exposed. Cases are compared in order and the value is
/// emitted on the port of the first equal one; when none is equal, it is emitted on
/// `_`. An empty case value never matches, and so does an invalid one - a case set
/// at runtime is additionally reported as a configuration error, while an invalid one
/// loaded from a preset is only kept as never-matching. An invalid `key` is handled the
/// same way, and makes every input go to `_`.
///
/// Case values use the same literal syntax as the If and Switch conditions: numbers,
/// strings, booleans, `null` and regular expressions, written as JSON (`"abc"`, `10`,
/// `true`, `null`) or as `/pattern/`. A regex matches a string value in full (implicitly
/// anchored as `^(?:...)$`; use `(?i)` for case-insensitivity), and a non-string value
/// never matches it. Comparison is by type as well as value, so the case `10` matches
/// neither the string `"10"` nor `true`, but numbers compare through their numeric value,
/// so `10` matches both an integer and a float input.
///
/// The `key` is a dot-separated list of object keys, and only selects what is compared -
/// the value emitted is always the original input value. When `key` is omitted, the input
/// value itself is compared. A key that does not resolve (the input is not an object, a
/// key is missing, or an intermediate value is not an object) is compared as `null` rather
/// than raising an error, so a case of `null` catches a missing field.
///
/// # Ports
/// - Input `input`: Value to test.
/// - Output `0`..`n-1`: The input value, emitted on the port of the first equal case value.
/// - Output `_`: The input value, when no case value is equal.
///
/// # Configuration
/// - `key`: Dot-separated path to the value to compare. Empty compares the input value itself.
/// - `n`: Number of case values and numbered output ports, clamped to 1..=64 (default: 2)
/// - `c1`..`cn`: Case values, compared in order.
///
/// # Example
/// With `key` = `user.status`, `n` = 3, `c1` = `"error"`, `c2` = `/warn.*/` and `c3` = `null`,
/// the input `{"user": {"status": "error"}}` is emitted unchanged on `0`,
/// `{"user": {"status": "warning"}}` on `1`, `{"user": {}}` on `2` (a missing key compares
/// as null), and `{"user": {"status": "ok"}}` on `_`.
#[modular_agent(
    title = "Match",
    category = CATEGORY,
    inputs = [PORT_INPUT],
    outputs = ["0", "1", PORT_DEFAULT],
    string_config(name = CONFIG_KEY),
    integer_config(name = CONFIG_N, default = 2),
    // `c1`..`c2` match the default `n`, so a freshly placed agent already exposes them;
    // `update_numbered_spec` takes over once `n` changes.
    string_config(name = CONFIG_C1),
    string_config(name = CONFIG_C2),
    hint(color=5, height=2),
)]
struct MatchAgent {
    data: AgentData,
    n: usize,

    // Raw config strings, kept to detect config changes
    key_src: String,
    case_srcs: Vec<String>,

    // Optimization: pre-parsed key path and case values.
    // `key` is None when the path is invalid, so that nothing matches.
    key: Option<Vec<String>>,
    cases: Vec<Option<CondLit>>,
}

#[async_trait]
impl AsAgent for MatchAgent {
    fn new(ma: ModularAgent, id: String, mut spec: AgentSpec) -> Result<Self, AgentError> {
        let (n, extras, case_srcs) = update_numbered_spec(&mut spec, &[CONFIG_KEY])?;
        let key_src = extras.into_iter().next().unwrap_or_default();
        // An invalid key or case is kept as never-matching instead of blocking the load.
        let key = parse_path(&key_src).ok();
        let (cases, _) = parse_all(&case_srcs, load_lit);
        let data = AgentData::new(ma, id, spec);
        Ok(Self {
            data,
            n,
            key_src,
            case_srcs,
            key,
            cases,
        })
    }

    fn configs_changed(&mut self) -> Result<(), AgentError> {
        let (n, extras, case_srcs) = update_numbered_spec(&mut self.data.spec, &[CONFIG_KEY])?;
        let key_src = extras.into_iter().next().unwrap_or_default();
        if n == self.n && key_src == self.key_src && case_srcs == self.case_srcs {
            return Ok(());
        }

        // `update_numbered_spec` has already rewritten the spec, so the parsed state has
        // to be committed as a whole even when a key or a case is invalid; otherwise
        // `cases` could outlive the output ports it routes to. An invalid value is kept
        // as never-matching, like at load time, and the parse error is reported afterwards.
        let mut first_error = None;
        let key = match parse_path(&key_src) {
            Ok(key) => Some(key),
            Err(e) => {
                first_error = Some(e);
                None
            }
        };
        let (cases, case_error) = parse_all(&case_srcs, load_lit);
        let first_error = first_error.or(case_error);

        self.n = n;
        self.key_src = key_src;
        self.case_srcs = case_srcs;
        self.key = key;
        self.cases = cases;
        self.emit_agent_spec_updated();

        match first_error {
            Some(e) => Err(e),
            None => Ok(()),
        }
    }

    async fn process(
        &mut self,
        ctx: AgentContext,
        _port: String,
        value: AgentValue,
    ) -> Result<(), AgentError> {
        // `AgentValue` has drop glue, so a temporary cannot be promoted to a `'static`
        // reference; bind it to a local that outlives `target`.
        let unit = AgentValue::unit();
        let matched = match self.key.as_ref() {
            // An invalid key matches nothing, so everything goes to `_`.
            None => None,
            Some(path) => {
                let target = if path.is_empty() {
                    &value
                } else {
                    get_nested_value(&value, path).unwrap_or(&unit)
                };
                self.cases
                    .iter()
                    .position(|case| case.as_ref().is_some_and(|lit| eq_cond(lit, target)))
            }
        };

        match matched {
            Some(i) => self.output(ctx, i.to_string(), value).await,
            None => self.output(ctx, PORT_DEFAULT, value).await,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_cond_operator_longest_match() {
        assert_eq!(
            parse_cond(">= 10").expect("valid"),
            Cond {
                path: vec![],
                op: CondOp::Ge,
                lit: CondLit::Integer(10)
            }
        );
        assert_eq!(
            parse_cond("<=10").expect("valid"),
            Cond {
                path: vec![],
                op: CondOp::Le,
                lit: CondLit::Integer(10)
            }
        );
        assert_eq!(
            parse_cond("> 10").expect("valid"),
            Cond {
                path: vec![],
                op: CondOp::Gt,
                lit: CondLit::Integer(10)
            }
        );
        assert_eq!(
            parse_cond("< 10").expect("valid"),
            Cond {
                path: vec![],
                op: CondOp::Lt,
                lit: CondLit::Integer(10)
            }
        );
        assert_eq!(
            parse_cond("== \"abc\"").expect("valid"),
            Cond {
                path: vec![],
                op: CondOp::Eq,
                lit: CondLit::String("abc".to_string())
            }
        );
        assert_eq!(
            parse_cond("!= null").expect("valid"),
            Cond {
                path: vec![],
                op: CondOp::Ne,
                lit: CondLit::Null
            }
        );
        assert_eq!(
            parse_cond("== true").expect("valid"),
            Cond {
                path: vec![],
                op: CondOp::Eq,
                lit: CondLit::Boolean(true)
            }
        );
    }

    #[test]
    fn test_parse_cond_with_path() {
        assert_eq!(
            parse_cond("user.age >= 18").expect("valid"),
            Cond {
                path: vec!["user".to_string(), "age".to_string()],
                op: CondOp::Ge,
                lit: CondLit::Integer(18)
            }
        );
        assert_eq!(
            parse_cond("status == \"ok\"").expect("valid"),
            Cond {
                path: vec!["status".to_string()],
                op: CondOp::Eq,
                lit: CondLit::String("ok".to_string())
            }
        );
        // No path keeps the input value itself as the target
        assert_eq!(
            parse_cond("> 10").expect("valid").path,
            Vec::<String>::new()
        );
        // Segments are trimmed
        assert_eq!(
            parse_cond(" user . age == null").expect("valid").path,
            vec!["user".to_string(), "age".to_string()]
        );
    }

    #[test]
    fn test_parse_cond_literal_with_operator_chars() {
        // The operator scan stops before the literal, so `>` inside the string is safe
        assert_eq!(
            parse_cond("name == \"a>b\"").expect("valid"),
            Cond {
                path: vec!["name".to_string()],
                op: CondOp::Eq,
                lit: CondLit::String("a>b".to_string())
            }
        );
    }

    #[test]
    fn test_parse_path() {
        assert_eq!(parse_path("").expect("valid"), Vec::<String>::new());
        assert_eq!(parse_path("   ").expect("valid"), Vec::<String>::new());
        assert_eq!(
            parse_path("user.age").expect("valid"),
            vec!["user".to_string(), "age".to_string()]
        );
        // Segments are trimmed
        assert_eq!(
            parse_path(" user . age ").expect("valid"),
            vec!["user".to_string(), "age".to_string()]
        );
        // Empty segments
        assert!(parse_path(".a").is_err());
        assert!(parse_path("a.").is_err());
        assert!(parse_path("a..b").is_err());
    }

    #[test]
    fn test_parse_lit() {
        assert_eq!(
            parse_lit("\"abc\"").expect("valid"),
            CondLit::String("abc".to_string())
        );
        assert_eq!(parse_lit("10").expect("valid"), CondLit::Integer(10));
        assert_eq!(parse_lit("1.5").expect("valid"), CondLit::Number(1.5));
        assert_eq!(parse_lit("true").expect("valid"), CondLit::Boolean(true));
        assert_eq!(parse_lit("null").expect("valid"), CondLit::Null);
        assert_eq!(
            parse_lit(" /err.*/ ").expect("valid"),
            CondLit::Regex(CondRegex(Regex::new("^(?:err.*)$").unwrap()))
        );

        // Empty
        assert!(parse_lit("").is_err());
        // Unquoted string is not valid JSON
        assert!(parse_lit("abc").is_err());
        // Array and object literals are unsupported
        assert!(parse_lit("[1, 2]").is_err());
        assert!(parse_lit("{\"a\": 1}").is_err());
        // Unclosed / invalid regex
        assert!(parse_lit("/abc").is_err());
        assert!(parse_lit("/[/").is_err());
    }

    #[test]
    fn test_parse_cond_invalid_path() {
        // Empty path segments
        assert!(parse_cond(".a > 1").is_err());
        assert!(parse_cond("a. > 1").is_err());
        assert!(parse_cond("a..b > 1").is_err());
        // No operator at all
        assert!(parse_cond("user.age").is_err());
    }

    #[test]
    fn test_eval_cond_path() {
        let value = AgentValue::object(im::hashmap! {
            "user".to_string() => AgentValue::object(im::hashmap! {
                "age".to_string() => AgentValue::integer(20),
            }),
            "name".to_string() => AgentValue::string("a".to_string()),
        });

        assert!(eval_cond(
            &parse_cond("user.age > 18").expect("valid"),
            &value
        ));
        assert!(!eval_cond(
            &parse_cond("user.age > 20").expect("valid"),
            &value
        ));
        assert!(eval_cond(
            &parse_cond("name == \"a\"").expect("valid"),
            &value
        ));

        // A missing key evaluates as null: order comparison fails, `== null` matches
        assert!(!eval_cond(
            &parse_cond("missing > 10").expect("valid"),
            &value
        ));
        assert!(eval_cond(
            &parse_cond("missing == null").expect("valid"),
            &value
        ));
        // An intermediate value that is not an object is also null
        assert!(eval_cond(
            &parse_cond("name.age == null").expect("valid"),
            &value
        ));

        // A non-object input with a path evaluates as null
        let scalar = AgentValue::integer(20);
        assert!(!eval_cond(
            &parse_cond("user.age > 18").expect("valid"),
            &scalar
        ));
        assert!(eval_cond(
            &parse_cond("user.age == null").expect("valid"),
            &scalar
        ));
        // Without a path the scalar itself is still tested
        assert!(eval_cond(&parse_cond("> 18").expect("valid"), &scalar));
    }

    #[test]
    fn test_parse_cond_invalid_literal() {
        // Missing operator
        assert!(parse_cond("10").is_err());
        // Missing literal
        assert!(parse_cond(">").is_err());
        // Unquoted string is not valid JSON
        assert!(parse_cond("== abc").is_err());
        // Array and object literals are unsupported
        assert!(parse_cond("== [1, 2]").is_err());
        assert!(parse_cond("== {\"a\": 1}").is_err());
        // `=` is not a valid operator prefix
        assert!(parse_cond("=> 10").is_err());
    }

    #[test]
    fn test_parse_cond_rejects_order_with_bool_or_null() {
        assert!(parse_cond("> true").is_err());
        assert!(parse_cond(">= false").is_err());
        assert!(parse_cond("< null").is_err());
        assert!(parse_cond("<= null").is_err());
        // Equality with the same literals stays valid
        assert!(parse_cond("== true").is_ok());
        assert!(parse_cond("!= null").is_ok());
    }

    #[test]
    fn test_parse_cond_regex() {
        // `== /pattern/` yields a Regex literal, anchored as a full match
        assert_eq!(
            parse_cond("== /err.*/").expect("valid"),
            Cond {
                path: vec![],
                op: CondOp::Eq,
                lit: CondLit::Regex(CondRegex(Regex::new("^(?:err.*)$").unwrap())),
            }
        );
        // A path together with `!=`
        assert_eq!(
            parse_cond("status != /ok/").expect("valid"),
            Cond {
                path: vec!["status".to_string()],
                op: CondOp::Ne,
                lit: CondLit::Regex(CondRegex(Regex::new("^(?:ok)$").unwrap())),
            }
        );
        // A `/` inside the pattern needs no escaping; the last `/` closes the literal
        assert_eq!(
            parse_cond("== /a/b/").expect("valid"),
            Cond {
                path: vec![],
                op: CondOp::Eq,
                lit: CondLit::Regex(CondRegex(Regex::new("^(?:a/b)$").unwrap())),
            }
        );
        // Operator characters inside the pattern are taken literally
        assert_eq!(
            parse_cond("== /a=b/").expect("valid"),
            Cond {
                path: vec![],
                op: CondOp::Eq,
                lit: CondLit::Regex(CondRegex(Regex::new("^(?:a=b)$").unwrap())),
            }
        );
        // `//` is the empty pattern, not an error
        assert_eq!(
            parse_cond("== //").expect("valid"),
            Cond {
                path: vec![],
                op: CondOp::Eq,
                lit: CondLit::Regex(CondRegex(Regex::new("^(?:)$").unwrap())),
            }
        );
    }

    #[test]
    fn test_parse_cond_regex_errors() {
        // Missing closing `/`
        assert!(parse_cond("== /abc").is_err());
        assert!(parse_cond("== /").is_err());
        // Invalid regex pattern
        assert!(parse_cond("== /[/").is_err());
        // An unbalanced pattern is rejected instead of merging with the `^(?:...)$`
        // anchoring wrapper into a valid but unanchored regex
        assert!(parse_cond("== /a)|(b/").is_err());
        assert!(parse_cond("== /)(/").is_err());
        // Order operators reject a regex literal at parse time
        assert!(parse_cond("> /a/").is_err());
        assert!(parse_cond(">= /a/").is_err());
    }

    #[test]
    fn test_eval_cond_regex() {
        // Full match: `/err.*/` matches "error" but not the substring in "my error"
        let cond = parse_cond("== /err.*/").expect("valid");
        assert!(eval_cond(&cond, &AgentValue::string("error")));
        assert!(!eval_cond(&cond, &AgentValue::string("my error")));

        // `!=` is the exact negation
        let ne = parse_cond("!= /err.*/").expect("valid");
        assert!(!eval_cond(&ne, &AgentValue::string("error")));
        assert!(eval_cond(&ne, &AgentValue::string("my error")));

        // `(?i)` makes the match case-insensitive
        let ci = parse_cond("== /(?i)error/").expect("valid");
        assert!(eval_cond(&ci, &AgentValue::string("ERROR")));

        // Path-based match
        let value = AgentValue::object(im::hashmap! {
            "status".to_string() => AgentValue::string("error".to_string()),
        });
        assert!(eval_cond(
            &parse_cond("status == /err.*/").expect("valid"),
            &value
        ));
        assert!(!eval_cond(
            &parse_cond("status == /ok/").expect("valid"),
            &value
        ));

        // The empty pattern `//` matches only the empty string
        let empty = parse_cond("== //").expect("valid");
        assert!(eval_cond(&empty, &AgentValue::string("")));
        assert!(!eval_cond(&empty, &AgentValue::string("a")));

        // A non-string target never matches: `==` is false, `!=` is true
        let num = AgentValue::integer(10);
        assert!(!eval_cond(&parse_cond("== /10/").expect("valid"), &num));
        assert!(eval_cond(&parse_cond("!= /10/").expect("valid"), &num));

        // An unresolved path is unit, which is not a string either
        let obj = AgentValue::object(im::hashmap! {
            "name".to_string() => AgentValue::string("a".to_string()),
        });
        assert!(!eval_cond(
            &parse_cond("missing == /.*/").expect("valid"),
            &obj
        ));
        assert!(eval_cond(
            &parse_cond("missing != /.*/").expect("valid"),
            &obj
        ));
    }
}
