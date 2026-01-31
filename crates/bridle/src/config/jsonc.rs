//! JSONC parsing utilities for OpenCode config files.

pub fn strip_jsonc_comments(input: &str) -> String {
    let mut result = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    let mut in_string = false;
    let mut escape_next = false;

    while let Some(c) = chars.next() {
        if escape_next {
            result.push(c);
            escape_next = false;
            continue;
        }

        if c == '\\' && in_string {
            result.push(c);
            escape_next = true;
            continue;
        }

        if c == '"' && !escape_next {
            in_string = !in_string;
            result.push(c);
            continue;
        }

        if !in_string && c == '/' {
            match chars.peek() {
                Some('/') => {
                    chars.next();
                    while let Some(&ch) = chars.peek() {
                        if ch == '\n' {
                            break;
                        }
                        chars.next();
                    }
                }
                Some('*') => {
                    chars.next();
                    while let Some(ch) = chars.next() {
                        if ch == '*' && chars.peek() == Some(&'/') {
                            chars.next();
                            break;
                        }
                    }
                }
                _ => result.push(c),
            }
        } else {
            result.push(c);
        }
    }
    strip_trailing_commas(&result)
}

fn strip_trailing_commas(input: &str) -> String {
    let mut result = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    let mut in_string = false;

    while let Some(c) = chars.next() {
        if c == '"' && !result.ends_with('\\') {
            in_string = !in_string;
            result.push(c);
            continue;
        }

        if !in_string && c == ',' {
            let mut lookahead = chars.clone();
            let has_trailing = loop {
                match lookahead.next() {
                    Some(ch) if ch.is_whitespace() => continue,
                    Some(']') | Some('}') => break true,
                    _ => break false,
                }
            };
            if !has_trailing {
                result.push(c);
            }
        } else {
            result.push(c);
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_line_comments() {
        let input = r#"{"key": "value" // comment
}"#;
        let result = strip_jsonc_comments(input);
        assert!(result.contains(r#""key": "value""#));
        assert!(!result.contains("comment"));
    }

    #[test]
    fn strips_block_comments() {
        let input = r#"{"key": /* block */ "value"}"#;
        let result = strip_jsonc_comments(input);
        assert_eq!(result, r#"{"key":  "value"}"#);
    }

    #[test]
    fn preserves_comments_in_strings() {
        let input = r#"{"key": "value // not a comment"}"#;
        let result = strip_jsonc_comments(input);
        assert_eq!(result, input);
    }

    #[test]
    fn strips_trailing_commas() {
        let input = r#"{"a": 1, "b": 2,}"#;
        let result = strip_jsonc_comments(input);
        assert_eq!(result, r#"{"a": 1, "b": 2}"#);
    }
}
