//! Transcript post-processing for command dictation.

pub fn normalize_transcript(raw: &str) -> String {
    let text = raw.trim();
    if text.is_empty() {
        return String::new();
    }

    let (command, is_command) = extract_command(text);
    if is_command || looks_like_command(command) {
        normalize_command(command)
    } else {
        text.to_string()
    }
}

fn extract_command(text: &str) -> (&str, bool) {
    let lower = text.to_lowercase();

    for marker in [" the command ", " command "] {
        if let Some(index) = lower.rfind(marker) {
            let before = &lower[..index];
            if before.contains("output")
                || before.contains("type")
                || before.contains("write")
                || before.contains("run")
                || before.contains("execute")
            {
                return (trim_command_edges(&text[index + marker.len()..]), true);
            }
        }
    }

    if let Some(rest) = lower.strip_prefix("command ") {
        let start = text.len() - rest.len();
        return (trim_command_edges(&text[start..]), true);
    }

    (text, false)
}

fn trim_command_edges(text: &str) -> &str {
    text.trim()
        .trim_start_matches(|c: char| c == ':' || c == ',' || c == '-' || c.is_whitespace())
        .trim_end_matches(|c: char| matches!(c, '.' | '!' | '?' | ',') || c.is_whitespace())
}

fn looks_like_command(text: &str) -> bool {
    let Some(first) = text.split_whitespace().next() else {
        return false;
    };
    matches!(
        clean_word(first).as_str(),
        "bun"
            | "cargo"
            | "cd"
            | "cp"
            | "deno"
            | "docker"
            | "git"
            | "go"
            | "kubectl"
            | "ls"
            | "mkdir"
            | "mv"
            | "node"
            | "npm"
            | "npx"
            | "pnpm"
            | "python"
            | "python3"
            | "rm"
            | "rustc"
            | "ssh"
            | "uv"
            | "uvx"
            | "yarn"
    )
}

fn normalize_command(text: &str) -> String {
    let mut out: Vec<String> = Vec::new();
    let words: Vec<String> = trim_command_edges(text)
        .split_whitespace()
        .map(clean_word)
        .filter(|word| !word.is_empty())
        .collect();

    let mut i = 0;
    while i < words.len() {
        let word = words[i].as_str();

        if word == "double"
            && words
                .get(i + 1)
                .is_some_and(|next| is_dash_word(next.as_str()))
        {
            push_double_dash(&mut out, words.get(i + 2).map(String::as_str));
            i += if words.get(i + 2).is_some() { 3 } else { 2 };
            continue;
        }

        if is_dash_word(word) {
            let mut count = 1;
            while words
                .get(i + count)
                .is_some_and(|next| is_dash_word(next.as_str()))
            {
                count += 1;
            }

            if count >= 2 {
                push_double_dash(&mut out, words.get(i + count).map(String::as_str));
                i += count + usize::from(words.get(i + count).is_some());
            } else if let Some(next) = words.get(i + 1) {
                out.push(format!("-{next}"));
                i += 2;
            } else {
                out.push("-".to_string());
                i += 1;
            }
            continue;
        }

        if matches!(word, "dot" | "period" | "slash" | "underscore" | "colon") {
            if let Some(next) = words.get(i + 1) {
                let symbol = match word {
                    "slash" => "/",
                    "underscore" => "_",
                    "colon" => ":",
                    _ => ".",
                };

                if let Some(previous) = out.pop() {
                    out.push(format!("{previous}{symbol}{next}"));
                } else {
                    out.push(format!("{symbol}{next}"));
                }
                i += 2;
                continue;
            }
        }

        out.push(words[i].clone());
        i += 1;
    }

    out.join(" ")
}

fn push_double_dash(out: &mut Vec<String>, next: Option<&str>) {
    match next {
        Some(next) => out.push(format!("--{next}")),
        None => out.push("--".to_string()),
    }
}

fn is_dash_word(word: &str) -> bool {
    matches!(word, "dash" | "hyphen" | "minus")
}

fn clean_word(word: &str) -> String {
    word.trim_matches(|c: char| {
        c.is_whitespace() || matches!(c, '"' | '\'' | '`' | ',' | '.' | '!' | '?' | ';')
    })
    .to_lowercase()
}

#[cfg(test)]
mod tests {
    use super::normalize_transcript;

    #[test]
    fn normalizes_command_prefix_and_double_dash_flag() {
        assert_eq!(
            normalize_transcript("I want you to output the command cargo run dash dash release."),
            "cargo run --release"
        );
    }

    #[test]
    fn normalizes_direct_command_dictation() {
        assert_eq!(
            normalize_transcript("cargo run dash dash release dash dash bin whispr"),
            "cargo run --release --bin whispr"
        );
    }

    #[test]
    fn leaves_regular_dictation_alone() {
        assert_eq!(
            normalize_transcript("I want you to output the sentence dash dash release."),
            "I want you to output the sentence dash dash release."
        );
    }
}
