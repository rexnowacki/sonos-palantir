#[derive(Debug, PartialEq)]
pub enum Command {
    Play(String),
    /// (optional speaker alias/"all", volume 0-100)
    Volume(Option<String>, u8),
    GroupAll,
    Ungroup,
    Next,
    Prev,
    Sleep(u32),
    SleepCancel,
    Reload,
    Unknown(String),
}

pub fn parse(input: &str) -> Option<Command> {
    let input = input.trim();
    if input.is_empty() {
        return None;
    }
    let (cmd, rest) = input
        .split_once(' ')
        .map(|(c, r)| (c, r.trim()))
        .unwrap_or((input, ""));

    match cmd {
        "play" | "p" => Some(Command::Play(rest.to_string())),
        "vol" | "volume" => {
            // "vol 30"  OR  "vol cthulhu 30"  OR  "vol all 30"
            if let Ok(v) = rest.parse::<u8>() {
                Some(Command::Volume(None, v))
            } else if let Some((name, num_str)) = rest.split_once(' ') {
                num_str.parse::<u8>().ok().map(|v| Command::Volume(Some(name.to_string()), v))
            } else {
                None
            }
        }
        "group" => {
            if rest == "all" {
                Some(Command::GroupAll)
            } else {
                Some(Command::Unknown(input.to_string()))
            }
        }
        "ungroup" => Some(Command::Ungroup),
        "next" | "n" => Some(Command::Next),
        "prev" | "previous" => Some(Command::Prev),
        "sleep" => {
            if rest == "0" || rest == "cancel" {
                Some(Command::SleepCancel)
            } else {
                rest.parse::<u32>().ok().map(Command::Sleep)
            }
        }
        "reload" => Some(Command::Reload),
        _ => Some(Command::Unknown(input.to_string())),
    }
}

/// Given partial command input (without leading `:`), return ghost text to display.
/// `playlist_names` is a list of `favorite_name` strings for fuzzy matching.
/// `speaker_names` is a list of speaker alias/names for commands that target speakers.
pub fn autocomplete(input: &str, playlist_names: &[String], speaker_names: &[String]) -> Option<String> {
    if input.is_empty() {
        return None;
    }
    // If no space yet, complete the command name
    if !input.contains(' ') {
        let commands = [
            "play", "vol", "group all", "ungroup", "next", "prev",
            "sleep", "reload",
        ];
        for cmd in &commands {
            if cmd.starts_with(input) && *cmd != input {
                return Some(cmd[input.len()..].to_string());
            }
        }
        return None;
    }
    let (cmd, rest) = input.split_once(' ').unwrap();

    // :play <query> — fuzzy match against playlist names
    if (cmd == "play" || cmd == "p") && !rest.is_empty() {
        return fuzzy_complete(rest, playlist_names);
    }

    // :vol <speaker> <number> — complete speaker name as first arg, append space for number
    if cmd == "vol" || cmd == "volume" {
        if !rest.contains(' ') && !rest.is_empty() {
            if rest.parse::<u8>().is_err() {
                let mut names = speaker_names.to_vec();
                names.push("all".to_string());
                if let Some(ghost) = fuzzy_complete(rest, &names) {
                    // Append trailing space so Tab gives ":vol cthulhu " ready for number
                    return Some(format!("{} ", ghost));
                }
            }
        }
    }

    None
}

/// Fuzzy-match `query` against `candidates`, returning ghost text suffix.
fn fuzzy_complete(query: &str, candidates: &[String]) -> Option<String> {
    let q = query.to_lowercase();
    // Exact match — nothing left to complete
    if candidates.iter().any(|n| n.to_lowercase() == q) {
        return None;
    }
    // Prefix match
    if let Some(m) = candidates.iter().find(|n| n.to_lowercase().starts_with(&q)) {
        let prefix_byte_len: usize = m.chars()
            .zip(m.to_lowercase().chars())
            .take(q.chars().count())
            .map(|(orig_c, _)| orig_c.len_utf8())
            .sum();
        return Some(m[prefix_byte_len..].to_string());
    }
    // Contains match fallback
    if let Some(m) = candidates.iter().find(|n| n.to_lowercase().contains(&q)) {
        return Some(format!(" → {}", m));
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_play() {
        assert_eq!(parse("play altwave"), Some(Command::Play("altwave".to_string())));
    }

    #[test]
    fn test_parse_volume() {
        assert_eq!(parse("vol 40"), Some(Command::Volume(None, 40)));
    }

    #[test]
    fn test_parse_volume_with_speaker() {
        assert_eq!(parse("vol cthulhu 30"), Some(Command::Volume(Some("cthulhu".to_string()), 30)));
    }

    #[test]
    fn test_parse_volume_all() {
        assert_eq!(parse("vol all 30"), Some(Command::Volume(Some("all".to_string()), 30)));
    }

    #[test]
    fn test_parse_group_all() {
        assert_eq!(parse("group all"), Some(Command::GroupAll));
    }

    #[test]
    fn test_parse_sleep() {
        assert_eq!(parse("sleep 30"), Some(Command::Sleep(30)));
    }

    #[test]
    fn test_parse_sleep_cancel() {
        assert_eq!(parse("sleep cancel"), Some(Command::SleepCancel));
        assert_eq!(parse("sleep 0"), Some(Command::SleepCancel));
    }

    #[test]
    fn test_parse_reload() {
        assert_eq!(parse("reload"), Some(Command::Reload));
    }

    #[test]
    fn test_parse_empty_returns_none() {
        assert_eq!(parse(""), None);
        assert_eq!(parse("   "), None);
    }

    #[test]
    fn test_parse_unknown() {
        assert!(matches!(parse("blorp"), Some(Command::Unknown(_))));
    }

    #[test]
    fn test_autocomplete_command_name() {
        assert_eq!(autocomplete("sl", &[], &[]), Some("eep".to_string()));
        assert_eq!(autocomplete("re", &[], &[]), Some("load".to_string()));
        assert_eq!(autocomplete("reload", &[], &[]), None); // exact match
    }

    #[test]
    fn test_autocomplete_play_fuzzy() {
        let names = vec!["Alt Wave".to_string(), "Jazz Classics".to_string()];
        let result = autocomplete("play alt", &names, &[]);
        assert_eq!(result, Some(" Wave".to_string()));
    }

    #[test]
    fn test_autocomplete_no_match() {
        let names = vec!["Alt Wave".to_string()];
        assert_eq!(autocomplete("play xyz", &names, &[]), None);
    }

    #[test]
    fn test_autocomplete_empty_input() {
        assert_eq!(autocomplete("", &[], &[]), None);
    }

    #[test]
    fn test_autocomplete_vol_speaker() {
        let speakers = vec!["cthulhu".to_string(), "family".to_string()];
        assert_eq!(autocomplete("vol cth", &[], &speakers), Some("ulhu ".to_string()));
        assert_eq!(autocomplete("vol fam", &[], &speakers), Some("ily ".to_string()));
        assert_eq!(autocomplete("vol al", &[], &speakers), Some("l ".to_string())); // "all"
    }

    #[test]
    fn test_autocomplete_vol_exact_match_returns_none() {
        let speakers = vec!["cthulhu".to_string()];
        // After completing, no more ghost text
        assert_eq!(autocomplete("vol cthulhu ", &[], &speakers), None);
    }

    #[test]
    fn test_autocomplete_vol_number_not_completed() {
        let speakers = vec!["cthulhu".to_string()];
        // Typing a number should not trigger speaker completion
        assert_eq!(autocomplete("vol 30", &[], &speakers), None);
    }

    #[test]
    fn test_parse_vol_no_arg_returns_none() {
        assert_eq!(parse("vol"), None);
    }

    #[test]
    fn test_parse_group_no_arg_returns_unknown() {
        assert!(matches!(parse("group"), Some(Command::Unknown(_))));
    }

    #[test]
    fn test_parse_play_alias_p() {
        assert_eq!(parse("p altwave"), Some(Command::Play("altwave".to_string())));
    }

    #[test]
    fn test_autocomplete_p_alias_plays_fuzzy() {
        let names = vec!["Alt Wave".to_string()];
        let result = autocomplete("p alt", &names, &[]);
        // "p alt" has a space so it enters the play-fuzzy path
        assert_eq!(result, Some(" Wave".to_string()));
    }
}
