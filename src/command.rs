//! Prompt lines. The same words the view keys send, plus breakpoint and watch edits.

use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    Resume,
    StepOver,
    StepIn,
    StepOut,
    ToggleBreakpoint,
    Break {
        path: PathBuf,
        line: i64,
        condition: Option<String>,
    },
    DeleteBreakpoint(i64),
    Watch(String),
    /// 1-based index into the watch list.
    Unwatch(usize),
    Thread(i64),
    /// 0-based stack index, matching the `#0` label.
    Frame(usize),
    Quit,
    Help,
}

pub fn parse(line: &str) -> Result<Option<Command>, String> {
    let line = line.trim();
    if line.is_empty() {
        return Ok(None);
    }
    let (word, rest) = line
        .split_once(char::is_whitespace)
        .map(|(word, rest)| (word, rest.trim()))
        .unwrap_or((line, ""));

    let command = match word {
        "c" | "cont" | "continue" => bare(rest, Command::Resume)?,
        "n" | "next" => bare(rest, Command::StepOver)?,
        "s" | "step" => bare(rest, Command::StepIn)?,
        "f" | "finish" => bare(rest, Command::StepOut)?,
        "q" | "quit" => bare(rest, Command::Quit)?,
        "help" | "?" => bare(rest, Command::Help)?,
        "break" | "b" => parse_break(rest)?,
        "delete" | "d" => Command::DeleteBreakpoint(parse_i64(rest, "id")?),
        "watch" | "w" => {
            if rest.is_empty() {
                return Err("watch needs an expression".into());
            }
            Command::Watch(rest.to_string())
        }
        "unwatch" => Command::Unwatch(parse_index(rest)?),
        "thread" | "t" => Command::Thread(parse_i64(rest, "thread id")?),
        "frame" => Command::Frame(parse_index(rest)?),
        other => return Err(format!("unknown command {other}")),
    };
    Ok(Some(command))
}

fn bare(rest: &str, command: Command) -> Result<Command, String> {
    if rest.is_empty() {
        Ok(command)
    } else {
        Err("unexpected arguments".into())
    }
}

fn parse_break(rest: &str) -> Result<Command, String> {
    if rest.is_empty() {
        return Err("break needs file:line".into());
    }
    let (loc, condition) = if let Some((loc, cond)) = rest.split_once(" if ") {
        let cond = cond.trim();
        if cond.is_empty() {
            return Err("break condition is empty".into());
        }
        (loc.trim(), Some(cond.to_string()))
    } else {
        (rest, None)
    };
    let (file, line) = loc
        .rsplit_once(':')
        .ok_or_else(|| "expected file:line".to_string())?;
    if file.is_empty() {
        return Err("expected file:line".into());
    }
    let line: i64 = line
        .trim()
        .parse()
        .map_err(|_| "expected a line number".to_string())?;
    if line < 1 {
        return Err("line numbers start at 1".into());
    }
    Ok(Command::Break {
        path: PathBuf::from(file),
        line,
        condition,
    })
}

fn parse_i64(rest: &str, name: &str) -> Result<i64, String> {
    if rest.is_empty() || rest.contains(char::is_whitespace) {
        return Err(format!("expected {name}"));
    }
    rest.parse().map_err(|_| format!("expected {name}"))
}

fn parse_index(rest: &str) -> Result<usize, String> {
    if rest.is_empty() || rest.contains(char::is_whitespace) {
        return Err("expected an index".into());
    }
    rest.parse().map_err(|_| "expected an index".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stepping_aliases() {
        assert_eq!(parse("c").unwrap(), Some(Command::Resume));
        assert_eq!(parse("continue").unwrap(), Some(Command::Resume));
        assert_eq!(parse("next").unwrap(), Some(Command::StepOver));
        assert_eq!(parse("s").unwrap(), Some(Command::StepIn));
        assert_eq!(parse("finish").unwrap(), Some(Command::StepOut));
        assert_eq!(parse("quit").unwrap(), Some(Command::Quit));
        assert_eq!(parse("  ").unwrap(), None);
    }

    #[test]
    fn break_with_condition() {
        let command = parse("break testdata/hello.c:14 if x == 1").unwrap();
        assert_eq!(
            command,
            Some(Command::Break {
                path: PathBuf::from("testdata/hello.c"),
                line: 14,
                condition: Some("x == 1".into()),
            })
        );
    }

    #[test]
    fn watch_delete_and_unknown() {
        assert_eq!(
            parse("watch head->data").unwrap(),
            Some(Command::Watch("head->data".into()))
        );
        assert_eq!(parse("unwatch 2").unwrap(), Some(Command::Unwatch(2)));
        assert_eq!(
            parse("delete 4").unwrap(),
            Some(Command::DeleteBreakpoint(4))
        );
        assert_eq!(parse("frame 0").unwrap(), Some(Command::Frame(0)));
        assert!(parse("nope").is_err());
        assert!(parse("break").is_err());
    }
}
