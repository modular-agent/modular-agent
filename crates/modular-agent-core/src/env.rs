//! `${VAR}` expansion shared by every config file a host reads (workspace
//! `settings.json`, the llm crate's `models.json`), so they accept the same
//! syntax.

/// Replaces `${NAME}` (NAME = `[A-Za-z_][A-Za-z0-9_]*`) with `lookup(NAME)`;
/// `$${` yields a literal `${`. Anything else, such as `${}` or an unclosed
/// `${`, is kept as written. Returns the name of the first variable `lookup`
/// does not define.
///
/// Pass `|var| std::env::var(var).ok()` to expand from the process
/// environment.
pub fn expand_vars(
    s: &str,
    lookup: &impl Fn(&str) -> Option<String>,
) -> std::result::Result<String, String> {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find('$') {
        out.push_str(&rest[..i]);
        let tail = &rest[i..];
        if let Some(after) = tail.strip_prefix("$${") {
            out.push_str("${");
            rest = after;
        } else if let Some((name, after)) = tail
            .strip_prefix("${")
            .and_then(|t| t.split_once('}'))
            .filter(|(name, _)| is_var_name(name))
        {
            out.push_str(&lookup(name).ok_or_else(|| name.to_string())?);
            rest = after;
        } else {
            out.push('$');
            rest = &tail[1..];
        }
    }
    out.push_str(rest);
    Ok(out)
}

fn is_var_name(name: &str) -> bool {
    let mut chars = name.chars();
    chars
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lookup(var: &str) -> Option<String> {
        match var {
            "TOKEN" => Some("secret".into()),
            "EMPTY" => Some(String::new()),
            _ => None,
        }
    }

    #[test]
    fn expands_variables() {
        assert_eq!(expand_vars("${TOKEN}", &lookup), Ok("secret".into()));
        assert_eq!(
            expand_vars("a-${TOKEN}-${EMPTY}b", &lookup),
            Ok("a-secret-b".into())
        );
        assert_eq!(expand_vars("$${TOKEN}", &lookup), Ok("${TOKEN}".into()));
        assert_eq!(
            expand_vars("$5 ${} ${1X} ${open", &lookup),
            Ok("$5 ${} ${1X} ${open".into())
        );
        assert_eq!(expand_vars("${MISSING}", &lookup), Err("MISSING".into()));
    }
}
