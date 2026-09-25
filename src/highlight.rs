//! Line-oriented C highlighter. Block comments carry across lines.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenKind {
    Plain,
    Keyword,
    Number,
    String,
    Comment,
    Preprocessor,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Token {
    pub kind: TokenKind,
    pub text: String,
}

pub fn highlight_line(line: &str, in_block: &mut bool) -> Vec<Token> {
    let mut tokens = Vec::new();
    let mut i = 0;

    if *in_block {
        if let Some(end) = line.find("*/") {
            push(&mut tokens, TokenKind::Comment, &line[..=end + 1]);
            *in_block = false;
            i = end + 2;
        } else {
            push(&mut tokens, TokenKind::Comment, line);
            return tokens;
        }
    }

    let rest = &line[i..];
    let leading = rest.len() - rest.trim_start().len();
    if rest.trim_start().starts_with('#') {
        push(&mut tokens, TokenKind::Plain, &rest[..leading]);
        highlight_preprocessor(&rest[leading..], &mut tokens, in_block);
        return tokens;
    }

    while i < line.len() {
        let rest = &line[i..];
        if rest.starts_with("//") {
            push(&mut tokens, TokenKind::Comment, rest);
            break;
        }
        if rest.starts_with("/*") {
            if let Some(end) = rest.find("*/") {
                push(&mut tokens, TokenKind::Comment, &rest[..=end + 1]);
                i += end + 2;
                continue;
            }
            push(&mut tokens, TokenKind::Comment, rest);
            *in_block = true;
            break;
        }

        let ch = rest.chars().next().expect("i is on a char boundary");
        if ch == '"' || ch == '\'' {
            let end = scan_quoted(line, i, ch);
            push(&mut tokens, TokenKind::String, &line[i..end]);
            i = end;
            continue;
        }
        if ch.is_ascii_digit() {
            let end = scan_number(line, i);
            push(&mut tokens, TokenKind::Number, &line[i..end]);
            i = end;
            continue;
        }
        if ch == '_' || ch.is_ascii_alphabetic() {
            let end = scan_ident(line, i);
            let text = &line[i..end];
            let kind = if is_keyword(text) {
                TokenKind::Keyword
            } else {
                TokenKind::Plain
            };
            push(&mut tokens, kind, text);
            i = end;
            continue;
        }
        let len = ch.len_utf8();
        push(&mut tokens, TokenKind::Plain, &line[i..i + len]);
        i += len;
    }

    tokens
}

fn highlight_preprocessor(rest: &str, tokens: &mut Vec<Token>, in_block: &mut bool) {
    if let Some(rel) = rest.find("//") {
        push(tokens, TokenKind::Preprocessor, &rest[..rel]);
        push(tokens, TokenKind::Comment, &rest[rel..]);
        return;
    }
    if let Some(rel) = rest.find("/*") {
        push(tokens, TokenKind::Preprocessor, &rest[..rel]);
        let after = &rest[rel..];
        if let Some(end) = after.find("*/") {
            push(tokens, TokenKind::Comment, &after[..=end + 1]);
            let next = rel + end + 2;
            if next < rest.len() {
                push(tokens, TokenKind::Preprocessor, &rest[next..]);
            }
        } else {
            push(tokens, TokenKind::Comment, after);
            *in_block = true;
        }
        return;
    }
    push(tokens, TokenKind::Preprocessor, rest);
}

fn push(tokens: &mut Vec<Token>, kind: TokenKind, text: &str) {
    if text.is_empty() {
        return;
    }
    if let Some(last) = tokens.last_mut()
        && last.kind == kind
    {
        last.text.push_str(text);
        return;
    }
    tokens.push(Token {
        kind,
        text: text.to_string(),
    });
}

fn scan_quoted(line: &str, start: usize, quote: char) -> usize {
    let bytes = line.as_bytes();
    let mut i = start + quote.len_utf8();
    while i < bytes.len() {
        let ch = line[i..].chars().next().unwrap();
        if ch == '\\' {
            let next = i + ch.len_utf8();
            if next >= line.len() {
                return line.len();
            }
            let escaped = line[next..].chars().next().unwrap();
            i = next + escaped.len_utf8();
            continue;
        }
        i += ch.len_utf8();
        if ch == quote {
            break;
        }
    }
    i
}

fn scan_number(line: &str, start: usize) -> usize {
    let rest = &line[start..];
    let mut i = start;
    if rest.starts_with("0x") || rest.starts_with("0X") {
        i += 2;
        for ch in line[i..].chars() {
            if ch.is_ascii_hexdigit() {
                i += ch.len_utf8();
            } else {
                break;
            }
        }
        return skip_suffix(line, i);
    }
    for ch in line[i..].chars() {
        if ch.is_ascii_digit() {
            i += ch.len_utf8();
        } else {
            break;
        }
    }
    if line[i..].starts_with('.') {
        let after = i + 1;
        if line[after..].starts_with(|c: char| c.is_ascii_digit()) {
            i = after;
            for ch in line[i..].chars() {
                if ch.is_ascii_digit() {
                    i += ch.len_utf8();
                } else {
                    break;
                }
            }
        }
    }
    skip_suffix(line, i)
}

fn skip_suffix(line: &str, mut i: usize) -> usize {
    for ch in line[i..].chars() {
        if matches!(ch, 'u' | 'U' | 'l' | 'L') {
            i += ch.len_utf8();
        } else {
            break;
        }
    }
    i
}

fn scan_ident(line: &str, start: usize) -> usize {
    let mut i = start;
    for ch in line[start..].chars() {
        if ch == '_' || ch.is_ascii_alphanumeric() {
            i += ch.len_utf8();
        } else {
            break;
        }
    }
    i
}

fn is_keyword(word: &str) -> bool {
    matches!(
        word,
        "auto"
            | "break"
            | "case"
            | "char"
            | "const"
            | "continue"
            | "default"
            | "do"
            | "double"
            | "else"
            | "enum"
            | "extern"
            | "float"
            | "for"
            | "goto"
            | "if"
            | "inline"
            | "int"
            | "long"
            | "register"
            | "restrict"
            | "return"
            | "short"
            | "signed"
            | "sizeof"
            | "static"
            | "struct"
            | "switch"
            | "typedef"
            | "union"
            | "unsigned"
            | "void"
            | "volatile"
            | "while"
            | "_Bool"
            | "_Complex"
            | "_Imaginary"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds(line: &str) -> Vec<(TokenKind, String)> {
        let mut in_block = false;
        highlight_line(line, &mut in_block)
            .into_iter()
            .map(|token| (token.kind, token.text))
            .collect()
    }

    #[test]
    fn keywords_numbers_and_strings() {
        let tokens = kinds("int x = 10;");
        assert!(tokens.contains(&(TokenKind::Keyword, "int".into())));
        assert!(tokens.contains(&(TokenKind::Number, "10".into())));
        assert!(kinds("\"hi\"").contains(&(TokenKind::String, "\"hi\"".into())));
    }

    #[test]
    fn line_comment_and_preprocessor() {
        assert_eq!(
            kinds("// note"),
            vec![(TokenKind::Comment, "// note".into())]
        );
        assert_eq!(
            kinds("#include <stdio.h>"),
            vec![(TokenKind::Preprocessor, "#include <stdio.h>".into())]
        );
    }

    #[test]
    fn block_comment_crosses_lines() {
        let mut in_block = false;
        let first = highlight_line("int x; /* start", &mut in_block);
        assert!(in_block);
        assert_eq!(first.last().unwrap().kind, TokenKind::Comment);
        let second = highlight_line(" still */ return;", &mut in_block);
        assert!(!in_block);
        assert_eq!(second[0].kind, TokenKind::Comment);
        assert!(second.iter().any(|token| token.text == "return"));
    }
}
