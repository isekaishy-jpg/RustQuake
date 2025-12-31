// Token parsing similar to COM_Parse from Quake.

pub fn com_parse(input: &str) -> Option<(String, &str)> {
    let bytes = input.as_bytes();
    let mut i = 0;

    if bytes.is_empty() {
        return None;
    }

    loop {
        while i < bytes.len() && bytes[i] <= b' ' {
            i += 1;
        }
        if i >= bytes.len() {
            return None;
        }

        if bytes[i] == b'/' && i + 1 < bytes.len() && bytes[i + 1] == b'/' {
            while i < bytes.len() && bytes[i] != b'\n' {
                i += 1;
            }
            continue;
        }

        break;
    }

    if bytes[i] == b'"' {
        i += 1;
        let start = i;
        while i < bytes.len() && bytes[i] != b'"' {
            i += 1;
        }
        let token = String::from_utf8_lossy(&bytes[start..i]).to_string();
        if i < bytes.len() {
            i += 1;
        }
        return Some((token, &input[i..]));
    }

    let start = i;
    while i < bytes.len() && bytes[i] > b' ' {
        i += 1;
    }
    let token = String::from_utf8_lossy(&bytes[start..i]).to_string();
    Some((token, &input[i..]))
}

pub fn com_tokenize(mut input: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    while let Some((token, rest)) = com_parse(input) {
        tokens.push(token);
        input = rest;
    }
    tokens
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_words_and_quotes() {
        let input = "  foo \"bar baz\" qux";
        let tokens = com_tokenize(input);
        assert_eq!(tokens, vec!["foo", "bar baz", "qux"]);
    }

    #[test]
    fn skips_comments() {
        let input = "foo // comment\nbar";
        let tokens = com_tokenize(input);
        assert_eq!(tokens, vec!["foo", "bar"]);
    }
}
