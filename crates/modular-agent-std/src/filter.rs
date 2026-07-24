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
const PORT_DEFAULT: &str = "default";

const CONFIG_COND: &str = "cond";
const CONFIG_N: &str = "n";
const CONFIG_COND1: &str = "cond1";
const CONFIG_COND2: &str = "cond2";

/// Upper bound for the Match agent's `n` config.
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

    let path_src = path_src.trim();
    let path = if path_src.is_empty() {
        Vec::new()
    } else {
        let mut path = Vec::new();
        for segment in path_src.split('.') {
            let segment = segment.trim();
            if segment.is_empty() {
                return Err(AgentError::InvalidConfig(format!(
                    "Condition path has an empty segment: {}",
                    path_src
                )));
            }
            path.push(segment.to_string());
        }
        path
    };

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

    // A regex literal is `/pattern/`. It is handled before JSON parsing so that a leading
    // `/` is never read as an (invalid) JSON value. The pattern is everything between the
    // first and last `/`; requiring the closing `/` to be the last character means a `/`
    // inside the pattern needs no escaping (`/a/b/` is the pattern `a/b`).
    let lit = if let Some(after_open) = rest.strip_prefix('/') {
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
        CondLit::Regex(CondRegex(re))
    } else {
        let json: serde_json::Value = serde_json::from_str(rest).map_err(|e| {
            AgentError::InvalidConfig(format!("Invalid condition literal `{}`: {}", rest, e))
        })?;

        match json {
            serde_json::Value::Null => CondLit::Null,
            serde_json::Value::Bool(b) => CondLit::Boolean(b),
            serde_json::Value::Number(n) => {
                if let Some(i) = n.as_i64() {
                    CondLit::Integer(i)
                } else if let Some(f) = n.as_f64() {
                    CondLit::Number(f)
                } else {
                    return Err(AgentError::InvalidConfig(format!(
                        "Condition literal is not representable as a number: {}",
                        rest
                    )));
                }
            }
            serde_json::Value::String(s) => CondLit::String(s),
            _ => {
                return Err(AgentError::InvalidConfig(format!(
                    "Array and object condition literals are not supported: {}",
                    rest
                )));
            }
        }
    };

    if op.is_order() && matches!(lit, CondLit::Boolean(_) | CondLit::Null | CondLit::Regex(_)) {
        return Err(AgentError::InvalidConfig(format!(
            "Order comparison is not supported for boolean, null or regex literals: {}",
            src
        )));
    }

    Ok(Cond { path, op, lit })
}

/// Parses a condition config value. An empty or blank config yields `None`.
fn load_cond(src: &str) -> Result<Option<Cond>, AgentError> {
    if src.trim().is_empty() {
        return Ok(None);
    }
    parse_cond(src).map(Some)
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

/// Routes the input value to the first of n conditions that matches.
///
/// The `n` config controls how many `cond1`..`condn` conditions exist, and how many
/// numbered output ports `0`..`n-1` are exposed. Conditions are evaluated in order and
/// the value is emitted on the port matching the first successful condition; when none
/// matches, it is emitted on `default`. An empty condition never matches, and so does an
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
/// - Output `default`: The input value, when no condition matches.
///
/// # Configuration
/// - `n`: Number of conditions and numbered output ports, clamped to 1..=64 (default: 2)
/// - `cond1`..`condn`: Condition expressions, evaluated in order.
///
/// # Example
/// With `n` = 2, `cond1` = `status == "error"` and `cond2` = `retry > 3`, the input
/// `{"status": "error", "retry": 0}` is emitted unchanged on `0`, `{"status": "ok",
/// "retry": 5}` on `1`, and `{"status": "ok", "retry": 0}` on `default`.
#[modular_agent(
    title = "Match",
    category = CATEGORY,
    inputs = [PORT_INPUT],
    outputs = ["0", "1", PORT_DEFAULT],
    integer_config(name = CONFIG_N, default = 2),
    // `cond1`..`cond2` match the default `n`, so a freshly placed agent already exposes
    // them; `update_spec` takes over once `n` changes.
    string_config(name = CONFIG_COND1),
    string_config(name = CONFIG_COND2),
    hint(color=5),
)]
struct MatchAgent {
    data: AgentData,
    n: usize,

    // Raw condition strings, kept to detect config changes
    cond_srcs: Vec<String>,

    // Optimization: Pre-parsed conditions
    conds: Vec<Option<Cond>>,
}

impl MatchAgent {
    fn update_spec(spec: &mut AgentSpec) -> Result<(usize, Vec<String>), AgentError> {
        let n = spec
            .configs
            .as_ref()
            .map(|cfg| cfg.get_integer_or(CONFIG_N, 2))
            .unwrap_or(2);
        // The upper bound keeps a stray config value from requesting a huge allocation.
        let n = n.clamp(1, MAX_N) as usize;

        // Dynamic generation of config definitions (ConfigSpecs)
        let mut configs = AgentConfigs::new();
        let mut config_specs = AgentConfigSpecs::default();

        // Re-set required configurations
        configs.set(CONFIG_N.to_string(), AgentValue::integer(n as i64));
        let Some(n_spec) = spec
            .config_specs
            .as_ref()
            .and_then(|cs| cs.get(CONFIG_N))
            .cloned()
        else {
            return Err(AgentError::InvalidConfig("config n must be present".into()));
        };
        config_specs.insert(CONFIG_N.to_string(), n_spec);

        let mut cond_srcs = Vec::with_capacity(n);
        for i in 1..=n {
            let cond_name = format!("cond{}", i);
            // `AgentDefinition::reconcile_spec` moves every config the definition does not
            // declare - which includes the dynamic `cond1`..`condn` - to a `_`-prefixed key
            // when a preset is loaded. Fall back to it so saved conditions survive a reload.
            let v = spec
                .configs
                .as_ref()
                .map(|cfg| {
                    if cfg.contains_key(&cond_name) {
                        cfg.get_string_or(&cond_name, "")
                    } else {
                        cfg.get_string_or(&format!("_{}", cond_name), "")
                    }
                })
                .unwrap_or_default();

            cond_srcs.push(v.clone());

            configs.set(cond_name.clone(), AgentValue::string(v));
            config_specs.insert(
                cond_name,
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

        Ok((n, cond_srcs))
    }
}

#[async_trait]
impl AsAgent for MatchAgent {
    fn new(ma: ModularAgent, id: String, mut spec: AgentSpec) -> Result<Self, AgentError> {
        let (n, cond_srcs) = Self::update_spec(&mut spec)?;
        // Invalid conditions are kept as never-matching instead of blocking the load.
        let conds = cond_srcs
            .iter()
            .map(|src| load_cond(src).unwrap_or(None))
            .collect();
        let data = AgentData::new(ma, id, spec);
        Ok(Self {
            data,
            n,
            cond_srcs,
            conds,
        })
    }

    fn configs_changed(&mut self) -> Result<(), AgentError> {
        let (n, cond_srcs) = Self::update_spec(&mut self.data.spec)?;
        if n == self.n && cond_srcs == self.cond_srcs {
            return Ok(());
        }

        // `update_spec` has already rewritten the spec, so the parsed state has to be
        // committed as a whole even when a condition is invalid; otherwise `conds` could
        // outlive the output ports it routes to. An invalid condition is kept as
        // never-matching, like at load time, and the parse error is reported afterwards.
        let mut first_error = None;
        let mut conds = Vec::with_capacity(cond_srcs.len());
        for src in &cond_srcs {
            match load_cond(src) {
                Ok(cond) => conds.push(cond),
                Err(e) => {
                    conds.push(None);
                    first_error.get_or_insert(e);
                }
            }
        }

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
