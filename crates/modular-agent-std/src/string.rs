use handlebars::Handlebars;
use im::vector;
use modular_agent_core::{
    AsModule, Error, ModularAgent, Module, ModuleContext, ModuleData, ModuleOutput, ModuleSpec,
    Result, Value, async_trait, modular_agent,
};
use regex::Regex;

const CATEGORY: &str = "Std/String";

const PORT_STRING: &str = "string";
const PORT_STRINGS: &str = "strings";
const PORT_UNMATCHED: &str = "unmatched";
const PORT_VALUE: &str = "value";
const PORT_T: &str = "t";
const PORT_F: &str = "f";

const CONFIG_LEN: &str = "len";
const CONFIG_OVERLAP: &str = "overlap";
const CONFIG_REGEX: &str = "regex";
const CONFIG_REPLACEMENT: &str = "replacement";
const CONFIG_SEP: &str = "sep";
const CONFIG_TEMPLATE: &str = "template";

/// Check if the input is a string.
#[modular_agent(
    title = "IsString",
    category = CATEGORY,
    inputs = [PORT_VALUE],
    outputs = [PORT_T, PORT_F],
    hint(color=5),
)]
struct IsStringModule {
    data: ModuleData,
}

#[async_trait]
impl AsModule for IsStringModule {
    fn new(ma: ModularAgent, id: String, spec: ModuleSpec) -> Result<Self> {
        Ok(Self {
            data: ModuleData::new(ma, id, spec),
        })
    }

    async fn process(&mut self, ctx: ModuleContext, _port: String, value: Value) -> Result<()> {
        if value.is_string() {
            self.output(ctx, PORT_T, value).await
        } else {
            self.output(ctx, PORT_F, value).await
        }
    }
}

/// Check if the input string is empty.
#[modular_agent(
    title = "IsEmptyString",
    category = CATEGORY,
    inputs = [PORT_STRING],
    outputs = [PORT_T, PORT_F],
    hint(color=5),
)]
struct IsEmptyStringModule {
    data: ModuleData,
}

#[async_trait]
impl AsModule for IsEmptyStringModule {
    fn new(ma: ModularAgent, id: String, spec: ModuleSpec) -> Result<Self> {
        Ok(Self {
            data: ModuleData::new(ma, id, spec),
        })
    }

    async fn process(&mut self, ctx: ModuleContext, _port: String, value: Value) -> Result<()> {
        let is_empty = if let Some(s) = value.as_str() {
            s.is_empty()
        } else {
            false
        };
        if is_empty {
            self.output(ctx, PORT_T, value).await
        } else {
            self.output(ctx, PORT_F, value).await
        }
    }
}

/// The `StringJoinModule` is responsible for joining an array of strings into a single string
/// using a specified separator. It processes input value, applies transformations to handle
/// escape sequences (e.g., `\n`, `\t`), and outputs the resulting string.
///
/// # Configuration
/// - `sep`: Separator inserted between the joined strings. Escape sequences (`\n`, `\t`, `\r`, `\\`) are interpreted (default: `\n`, i.e. a newline).
///
/// # Input
/// - Expects an array of strings as input value.
///
/// # Output
/// - Produces a single joined string as output.
///
/// # Example
/// Given the input `["Hello", "World"]` and `sep` set to `" "`, the output will be `"Hello World"`.
#[modular_agent(
    title = "String Join",
    category = CATEGORY,
    inputs = [PORT_STRINGS],
    outputs = [PORT_STRING],
    string_config(name = CONFIG_SEP, default = "\\n"),
    hint(color=5),
)]
struct StringJoinModule {
    data: ModuleData,
}

#[async_trait]
impl AsModule for StringJoinModule {
    fn new(ma: ModularAgent, id: String, spec: ModuleSpec) -> Result<Self> {
        Ok(Self {
            data: ModuleData::new(ma, id, spec),
        })
    }

    async fn process(&mut self, ctx: ModuleContext, _port: String, value: Value) -> Result<()> {
        let config = self.configs()?;

        let sep = config.get_string_or_default(CONFIG_SEP);

        if value.is_array() {
            let mut out = Vec::new();
            for v in value
                .as_array()
                .ok_or_else(|| Error::InvalidArrayValue("Expected array".into()))?
            {
                out.push(v.as_str().unwrap_or_default());
            }
            let mut out = out.join(&sep);
            out = out.replace("\\n", "\n");
            out = out.replace("\\t", "\t");
            out = out.replace("\\r", "\r");
            out = out.replace("\\\\", "\\");
            let out_value = Value::string(out);
            self.output(ctx, PORT_STRING, out_value).await
        } else {
            self.output(ctx, PORT_STRING, value).await
        }
    }
}

#[modular_agent(
    title = "String Length Split",
    category = CATEGORY,
    inputs = [PORT_STRING],
    outputs = [PORT_STRINGS],
    integer_config(name = CONFIG_LEN, default = 65536),
    integer_config(name = CONFIG_OVERLAP, default = 1024),
    hint(color=5),
)]
struct StringLengthSplitModule {
    data: ModuleData,
}

#[async_trait]
impl AsModule for StringLengthSplitModule {
    fn new(ma: ModularAgent, id: String, spec: ModuleSpec) -> Result<Self> {
        Ok(Self {
            data: ModuleData::new(ma, id, spec),
        })
    }

    async fn process(&mut self, ctx: ModuleContext, _port: String, value: Value) -> Result<()> {
        let config = self.configs()?;

        let n = config.get_integer_or_default(CONFIG_LEN);
        if n <= 0 {
            return Err(Error::InvalidConfig("len must be greater than 0".into()));
        }
        let n = n as usize;

        let overlap = config.get_integer_or_default(CONFIG_OVERLAP) as usize;
        if overlap >= n {
            return Err(Error::InvalidConfig("overlap must be less than len".into()));
        }

        let s = value
            .as_str()
            .ok_or_else(|| Error::InvalidValue("Input value must be a string".into()))?;

        let mut out = Vec::new();
        let mut start = 0;
        let len = s.len();
        while start < len {
            let mut end = usize::min(start + n, len);
            while !s.is_char_boundary(end) {
                end -= 1;
            }
            if end <= start {
                end = start + s[start..].chars().next().map(|c| c.len_utf8()).unwrap_or(1);
            }

            out.push(Value::string(s[start..end].to_string()));

            if end == len {
                break;
            }

            let mut next_start = end.saturating_sub(overlap);
            while next_start < len && !s.is_char_boundary(next_start) {
                next_start += 1;
            }
            start = next_start;
        }
        self.output(ctx, PORT_STRINGS, Value::array(out.into()))
            .await
    }
}

// Template String Module
#[modular_agent(
    title = "Template String",
    category = CATEGORY,
    inputs = [PORT_VALUE],
    outputs = [PORT_STRING],
    string_config(name = CONFIG_TEMPLATE, default = "{{value}}"),
    hint(color=5),
)]
struct TemplateStringModule {
    data: ModuleData,
}

#[async_trait]
impl AsModule for TemplateStringModule {
    fn new(ma: ModularAgent, id: String, spec: ModuleSpec) -> Result<Self> {
        Ok(Self {
            data: ModuleData::new(ma, id, spec),
        })
    }

    async fn process(&mut self, ctx: ModuleContext, _port: String, value: Value) -> Result<()> {
        let config = self.configs()?;

        let template = config.get_string_or_default(CONFIG_TEMPLATE);
        if template.is_empty() {
            return Err(Error::InvalidConfig("template is not set".into()));
        }

        let reg = handlebars_new();

        if value.is_array() {
            let mut out_arr = Vec::new();
            for v in value
                .as_array()
                .ok_or_else(|| Error::InvalidArrayValue("Expected array".into()))?
            {
                let data = template_data(v);
                let rendered_string = reg.render_template(&template, &data).map_err(|e| {
                    Error::InvalidValue(format!("Failed to render template: {}", e))
                })?;
                out_arr.push(rendered_string.into());
            }
            self.output(ctx, PORT_STRING, Value::array(out_arr.into()))
                .await
        } else {
            let data = template_data(&value);
            let rendered_string = reg
                .render_template(&template, &data)
                .map_err(|e| Error::InvalidValue(format!("Failed to render template: {}", e)))?;
            let out_value = Value::string(rendered_string);
            self.output(ctx, PORT_STRING, out_value).await
        }
    }
}

// Template Text Module
#[modular_agent(
    title = "Template Text",
    category = CATEGORY,
    inputs = [PORT_VALUE],
    outputs = [PORT_STRING],
    text_config(name = CONFIG_TEMPLATE, default = "{{value}}"),
    hint(color=5),
)]
struct TemplateTextModule {
    data: ModuleData,
}

#[async_trait]
impl AsModule for TemplateTextModule {
    fn new(ma: ModularAgent, id: String, spec: ModuleSpec) -> Result<Self> {
        Ok(Self {
            data: ModuleData::new(ma, id, spec),
        })
    }

    async fn process(&mut self, ctx: ModuleContext, _port: String, value: Value) -> Result<()> {
        let config = self.configs()?;

        let template = config.get_string_or_default(CONFIG_TEMPLATE);
        if template.is_empty() {
            return Err(Error::InvalidConfig("template is not set".into()));
        }

        let reg = handlebars_new();

        if value.is_array() {
            let mut out_arr = Vec::new();
            for v in value
                .as_array()
                .ok_or_else(|| Error::InvalidArrayValue("Expected array".into()))?
            {
                let data = template_data(v);
                let rendered_string = reg.render_template(&template, &data).map_err(|e| {
                    Error::InvalidValue(format!("Failed to render template: {}", e))
                })?;
                out_arr.push(rendered_string.into());
            }
            self.output(ctx, PORT_STRING, Value::array(out_arr.into()))
                .await
        } else {
            let data = template_data(&value);
            let rendered_string = reg
                .render_template(&template, &data)
                .map_err(|e| Error::InvalidValue(format!("Failed to render template: {}", e)))?;
            let out_value = Value::string(rendered_string);
            self.output(ctx, PORT_STRING, out_value).await
        }
    }
}

// Template Array Module
#[modular_agent(
    title = "Template Array",
    category = CATEGORY,
    inputs = [PORT_VALUE],
    outputs = [PORT_STRING],
    text_config(name = CONFIG_TEMPLATE, default = "{{value}}"),
    hint(color=5),
)]
struct TemplateArrayModule {
    data: ModuleData,
}

#[async_trait]
impl AsModule for TemplateArrayModule {
    fn new(ma: ModularAgent, id: String, spec: ModuleSpec) -> Result<Self> {
        Ok(Self {
            data: ModuleData::new(ma, id, spec),
        })
    }

    async fn process(&mut self, ctx: ModuleContext, _port: String, value: Value) -> Result<()> {
        let config = self.configs()?;

        let template = config.get_string_or_default(CONFIG_TEMPLATE);
        if template.is_empty() {
            return Err(Error::InvalidConfig("template is not set".into()));
        }

        let reg = handlebars_new();

        if value.is_array() {
            let rendered_string = reg
                .render_template(&template, &value)
                .map_err(|e| Error::InvalidValue(format!("Failed to render template: {}", e)))?;
            self.output(ctx, PORT_STRING, Value::string(rendered_string))
                .await
        } else {
            let d = Value::array(vector![value.clone()]);
            let rendered_string = reg
                .render_template(&template, &d)
                .map_err(|e| Error::InvalidValue(format!("Failed to render template: {}", e)))?;
            let out_value = Value::string(rendered_string);
            self.output(ctx, PORT_STRING, out_value).await
        }
    }
}

/// Extracts the first substring matching a regular expression.
///
/// The pattern is searched unanchored, so it matches anywhere in the input string.
/// The whole match is emitted; capture groups do not affect the output. When the
/// pattern does not match, a unit value is emitted on `unmatched` instead, so a
/// workflow can branch on the missing case.
///
/// # Ports
/// - Input `string`: String to search. A non-string input is an error.
/// - Output `string`: The first matched substring.
/// - Output `unmatched`: A unit value, when the pattern does not match.
///
/// # Configuration
/// - `regex`: Regular expression to search for. Processing fails while it is empty or
///   invalid.
///
/// # Example
/// With `regex` set to `[0-9]+`, the input `"abc123def456"` emits `"123"` on `string`,
/// while `"abcdef"` emits a unit value on `unmatched`.
#[modular_agent(
    title = "Regex Match",
    category = CATEGORY,
    inputs = [PORT_STRING],
    outputs = [PORT_STRING, PORT_UNMATCHED],
    string_config(name = CONFIG_REGEX),
    hint(color=5),
)]
struct RegexMatchModule {
    data: ModuleData,
    regex: Option<Regex>,
}

#[async_trait]
impl AsModule for RegexMatchModule {
    fn new(ma: ModularAgent, id: String, spec: ModuleSpec) -> Result<Self> {
        // Keep an invalid regex from blocking the load of a patch; it is reported
        // on the first process() call instead.
        let regex = load_regex_config(&spec).unwrap_or(None);
        Ok(Self {
            data: ModuleData::new(ma, id, spec),
            regex,
        })
    }

    fn configs_changed(&mut self) -> Result<()> {
        // The config value is already committed when this is called, so the previous
        // regex must be dropped even when the new one fails to compile. Otherwise the
        // module would keep matching by a pattern the config no longer holds.
        match load_regex_config(&self.data.spec) {
            Ok(regex) => {
                self.regex = regex;
                Ok(())
            }
            Err(e) => {
                self.regex = None;
                Err(e)
            }
        }
    }

    async fn process(&mut self, ctx: ModuleContext, _port: String, value: Value) -> Result<()> {
        let Some(re) = self.regex.as_ref() else {
            return Err(Error::InvalidConfig(
                "config regex must be a valid regular expression".into(),
            ));
        };
        let s = value
            .as_str()
            .ok_or_else(|| Error::InvalidValue("Input value must be a string".into()))?;
        let matched = re.find(s).map(|m| m.as_str().to_string());
        match matched {
            Some(m) => self.output(ctx, PORT_STRING, Value::string(m)).await,
            None => self.output(ctx, PORT_UNMATCHED, Value::unit()).await,
        }
    }
}

/// Extracts every substring matching a regular expression.
///
/// The pattern is searched unanchored and matches do not overlap. Each whole match is
/// emitted; capture groups do not affect the output. When the pattern does not match
/// at all, a unit value is emitted on `unmatched` instead of an empty array, so a
/// workflow can branch on the missing case.
///
/// # Ports
/// - Input `string`: String to search. A non-string input is an error.
/// - Output `strings`: Array of all matched substrings, in input order.
/// - Output `unmatched`: A unit value, when the pattern does not match.
///
/// # Configuration
/// - `regex`: Regular expression to search for. Processing fails while it is empty or
///   invalid.
///
/// # Example
/// With `regex` set to `[0-9]+`, the input `"abc123def456"` emits `["123", "456"]` on
/// `strings`, while `"abcdef"` emits a unit value on `unmatched`.
#[modular_agent(
    title = "Regex Match All",
    category = CATEGORY,
    inputs = [PORT_STRING],
    outputs = [PORT_STRINGS, PORT_UNMATCHED],
    string_config(name = CONFIG_REGEX),
    hint(color=5),
)]
struct RegexMatchAllModule {
    data: ModuleData,
    regex: Option<Regex>,
}

#[async_trait]
impl AsModule for RegexMatchAllModule {
    fn new(ma: ModularAgent, id: String, spec: ModuleSpec) -> Result<Self> {
        // Keep an invalid regex from blocking the load of a patch; it is reported
        // on the first process() call instead.
        let regex = load_regex_config(&spec).unwrap_or(None);
        Ok(Self {
            data: ModuleData::new(ma, id, spec),
            regex,
        })
    }

    fn configs_changed(&mut self) -> Result<()> {
        // The config value is already committed when this is called, so the previous
        // regex must be dropped even when the new one fails to compile. Otherwise the
        // module would keep matching by a pattern the config no longer holds.
        match load_regex_config(&self.data.spec) {
            Ok(regex) => {
                self.regex = regex;
                Ok(())
            }
            Err(e) => {
                self.regex = None;
                Err(e)
            }
        }
    }

    async fn process(&mut self, ctx: ModuleContext, _port: String, value: Value) -> Result<()> {
        let Some(re) = self.regex.as_ref() else {
            return Err(Error::InvalidConfig(
                "config regex must be a valid regular expression".into(),
            ));
        };
        let s = value
            .as_str()
            .ok_or_else(|| Error::InvalidValue("Input value must be a string".into()))?;
        let matches: Vec<Value> = re
            .find_iter(s)
            .map(|m| Value::string(m.as_str().to_string()))
            .collect();
        if matches.is_empty() {
            self.output(ctx, PORT_UNMATCHED, Value::unit()).await
        } else {
            self.output(ctx, PORT_STRINGS, Value::array(matches.into()))
                .await
        }
    }
}

/// Replaces the first substring matching a regular expression.
///
/// The pattern is searched unanchored, so it matches anywhere in the input string.
/// Only the first match is replaced; the rest of the string is left untouched. The
/// replacement may reference capture groups as `$1` or `${name}`, and a literal `$`
/// is written as `$$`. When the pattern does not match, the input string is emitted
/// unchanged.
///
/// # Ports
/// - Input `string`: String to rewrite. A non-string input is an error.
/// - Output `string`: The rewritten string.
///
/// # Configuration
/// - `regex`: Regular expression to search for. Processing fails while it is empty or
///   invalid.
/// - `replacement`: Replacement text. May reference capture groups (default: empty,
///   which deletes the match).
///
/// # Example
/// With `regex` set to `[0-9]+` and `replacement` set to `#`, the input
/// `"abc123def456"` emits `"abc#def456"`, while `"abcdef"` is emitted unchanged.
#[modular_agent(
    title = "Regex Replace",
    category = CATEGORY,
    inputs = [PORT_STRING],
    outputs = [PORT_STRING],
    string_config(name = CONFIG_REGEX),
    string_config(name = CONFIG_REPLACEMENT, default = ""),
    hint(color=5),
)]
struct RegexReplaceModule {
    data: ModuleData,
    regex: Option<Regex>,
}

#[async_trait]
impl AsModule for RegexReplaceModule {
    fn new(ma: ModularAgent, id: String, spec: ModuleSpec) -> Result<Self> {
        // Keep an invalid regex from blocking the load of a patch; it is reported
        // on the first process() call instead.
        let regex = load_regex_config(&spec).unwrap_or(None);
        Ok(Self {
            data: ModuleData::new(ma, id, spec),
            regex,
        })
    }

    fn configs_changed(&mut self) -> Result<()> {
        // The config value is already committed when this is called, so the previous
        // regex must be dropped even when the new one fails to compile. Otherwise the
        // module would keep matching by a pattern the config no longer holds.
        match load_regex_config(&self.data.spec) {
            Ok(regex) => {
                self.regex = regex;
                Ok(())
            }
            Err(e) => {
                self.regex = None;
                Err(e)
            }
        }
    }

    async fn process(&mut self, ctx: ModuleContext, _port: String, value: Value) -> Result<()> {
        let Some(re) = self.regex.as_ref() else {
            return Err(Error::InvalidConfig(
                "config regex must be a valid regular expression".into(),
            ));
        };
        let replacement = self.configs()?.get_string_or_default(CONFIG_REPLACEMENT);
        let s = value
            .as_str()
            .ok_or_else(|| Error::InvalidValue("Input value must be a string".into()))?;
        let out = re.replace(s, replacement.as_str()).into_owned();
        self.output(ctx, PORT_STRING, Value::string(out)).await
    }
}

/// Replaces every substring matching a regular expression.
///
/// The pattern is searched unanchored and matches do not overlap. The replacement may
/// reference capture groups as `$1` or `${name}`, and a literal `$` is written as
/// `$$`. When the pattern does not match, the input string is emitted unchanged.
///
/// # Ports
/// - Input `string`: String to rewrite. A non-string input is an error.
/// - Output `string`: The rewritten string.
///
/// # Configuration
/// - `regex`: Regular expression to search for. Processing fails while it is empty or
///   invalid.
/// - `replacement`: Replacement text. May reference capture groups (default: empty,
///   which deletes the matches).
///
/// # Example
/// With `regex` set to `[0-9]+` and `replacement` set to `#`, the input
/// `"abc123def456"` emits `"abc#def#"`, while `"abcdef"` is emitted unchanged.
#[modular_agent(
    title = "Regex Replace All",
    category = CATEGORY,
    inputs = [PORT_STRING],
    outputs = [PORT_STRING],
    string_config(name = CONFIG_REGEX),
    string_config(name = CONFIG_REPLACEMENT, default = ""),
    hint(color=5),
)]
struct RegexReplaceAllModule {
    data: ModuleData,
    regex: Option<Regex>,
}

#[async_trait]
impl AsModule for RegexReplaceAllModule {
    fn new(ma: ModularAgent, id: String, spec: ModuleSpec) -> Result<Self> {
        // Keep an invalid regex from blocking the load of a patch; it is reported
        // on the first process() call instead.
        let regex = load_regex_config(&spec).unwrap_or(None);
        Ok(Self {
            data: ModuleData::new(ma, id, spec),
            regex,
        })
    }

    fn configs_changed(&mut self) -> Result<()> {
        // The config value is already committed when this is called, so the previous
        // regex must be dropped even when the new one fails to compile. Otherwise the
        // module would keep matching by a pattern the config no longer holds.
        match load_regex_config(&self.data.spec) {
            Ok(regex) => {
                self.regex = regex;
                Ok(())
            }
            Err(e) => {
                self.regex = None;
                Err(e)
            }
        }
    }

    async fn process(&mut self, ctx: ModuleContext, _port: String, value: Value) -> Result<()> {
        let Some(re) = self.regex.as_ref() else {
            return Err(Error::InvalidConfig(
                "config regex must be a valid regular expression".into(),
            ));
        };
        let replacement = self.configs()?.get_string_or_default(CONFIG_REPLACEMENT);
        let s = value
            .as_str()
            .ok_or_else(|| Error::InvalidValue("Input value must be a string".into()))?;
        let out = re.replace_all(s, replacement.as_str()).into_owned();
        self.output(ctx, PORT_STRING, Value::string(out)).await
    }
}

fn load_regex_config(spec: &ModuleSpec) -> Result<Option<Regex>> {
    let src = spec
        .configs
        .as_ref()
        .map(|cfg| cfg.get_string_or_default(CONFIG_REGEX))
        .unwrap_or_default();
    if src.is_empty() {
        return Ok(None);
    }
    let re = Regex::new(&src)
        .map_err(|e| Error::InvalidConfig(format!("Invalid regex `{}`: {}", src, e)))?;
    Ok(Some(re))
}

/// Build the template root: an object's entries are exposed at the top level so
/// `{{color}}` works, and `value` always holds the whole input (winning on collision)
/// so `{{value}}` / `{{value.color}}` keep their meaning.
fn template_data(value: &Value) -> serde_json::Value {
    let v = serde_json::to_value(value).unwrap_or(serde_json::Value::Null);
    let mut map = match &v {
        serde_json::Value::Object(obj) => obj.clone(),
        _ => serde_json::Map::new(),
    };
    map.insert("value".to_string(), v);
    serde_json::Value::Object(map)
}

fn handlebars_new<'a>() -> Handlebars<'a> {
    let mut reg = Handlebars::new();
    reg.register_escape_fn(handlebars::no_escape);
    reg.register_helper("to_json", Box::new(to_json_helper));

    #[cfg(feature = "yaml")]
    reg.register_helper("to_yaml", Box::new(to_yaml_helper));

    reg
}

fn to_json_helper(
    h: &handlebars::Helper<'_>,
    _: &handlebars::Handlebars<'_>,
    _: &handlebars::Context,
    _: &mut handlebars::RenderContext<'_, '_>,
    out: &mut dyn handlebars::Output,
) -> handlebars::HelperResult {
    if let Some(value) = h.param(0) {
        let json_str = serde_json::to_string_pretty(&value.value()).map_err(|e| {
            handlebars::RenderErrorReason::Other(format!("Failed to serialize to JSON: {}", e))
        })?;
        out.write(&json_str)?;
    }
    Ok(())
}

#[cfg(feature = "yaml")]
fn to_yaml_helper(
    h: &handlebars::Helper<'_>,
    _: &handlebars::Handlebars<'_>,
    _: &handlebars::Context,
    _: &mut handlebars::RenderContext<'_, '_>,
    out: &mut dyn handlebars::Output,
) -> handlebars::HelperResult {
    if let Some(value) = h.param(0) {
        let yaml_str = serde_yaml_ng::to_string(&value.value()).map_err(|e| {
            handlebars::RenderErrorReason::Other(format!("Failed to serialize to YAML: {}", e))
        })?;
        out.write(&yaml_str)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn test_template_data_object_spreads_entries() {
        let value = Value::from_json(json!({"color": "blue", "count": 3})).unwrap();
        let data = template_data(&value);
        assert_eq!(data["color"], json!("blue"));
        assert_eq!(data["count"], json!(3));
        assert_eq!(data["value"], json!({"color": "blue", "count": 3}));
    }

    #[test]
    fn test_template_data_value_key_wins_on_collision() {
        let value = Value::from_json(json!({"value": "inner", "x": 1})).unwrap();
        let data = template_data(&value);
        assert_eq!(data["x"], json!(1));
        assert_eq!(data["value"], json!({"value": "inner", "x": 1}));
    }

    #[test]
    fn test_template_data_non_object() {
        let data = template_data(&Value::string("hello"));
        assert_eq!(data, json!({"value": "hello"}));

        let data = template_data(&Value::integer(42));
        assert_eq!(data, json!({"value": 42}));
    }
}
