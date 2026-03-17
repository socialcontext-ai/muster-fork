use muster::TabProfile;

/// Parse a `name:cwd[:command]` string into a `TabProfile`.
pub(crate) fn parse_tab(input: &str) -> Result<TabProfile, String> {
    let parts: Vec<&str> = input.splitn(3, ':').collect();
    if parts.len() < 2 {
        return Err(format!(
            "invalid tab format '{input}': expected 'name:cwd' or 'name:cwd:command'"
        ));
    }
    let name = parts[0].to_string();
    let cwd = if parts[1] == "." {
        std::env::current_dir()
            .map_or_else(|_| ".".to_string(), |p| p.to_string_lossy().to_string())
    } else {
        parts[1].to_string()
    };
    let command = parts
        .get(2)
        .map(std::string::ToString::to_string)
        .filter(|s| !s.is_empty());
    Ok(TabProfile {
        name,
        cwd,
        command,
        layout: None,
        panes: vec![],
    })
}

/// Build tabs from `--tab` flags, defaulting to a single Shell tab at $HOME.
pub(crate) fn build_tabs(raw: &[String]) -> Result<Vec<TabProfile>, String> {
    if raw.is_empty() {
        let home = dirs::home_dir()
            .map_or_else(|| "/tmp".to_string(), |p| p.to_string_lossy().to_string());
        return Ok(vec![TabProfile {
            name: "Shell".to_string(),
            cwd: home,
            command: None,
            layout: None,
            panes: vec![],
        }]);
    }
    raw.iter().map(|s| parse_tab(s)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_tab_name_and_cwd() {
        let tab = parse_tab("Shell:/home/user").unwrap();
        assert_eq!(tab.name, "Shell");
        assert_eq!(tab.cwd, "/home/user");
        assert!(tab.command.is_none());
    }

    #[test]
    fn parse_tab_with_command() {
        let tab = parse_tab("Dev:/home/user:npm run dev").unwrap();
        assert_eq!(tab.name, "Dev");
        assert_eq!(tab.cwd, "/home/user");
        assert_eq!(tab.command.as_deref(), Some("npm run dev"));
    }

    #[test]
    fn parse_tab_empty_command_becomes_none() {
        let tab = parse_tab("Shell:/home/user:").unwrap();
        assert!(tab.command.is_none());
    }

    #[test]
    fn parse_tab_missing_cwd_fails() {
        assert!(parse_tab("Shell").is_err());
    }
}
