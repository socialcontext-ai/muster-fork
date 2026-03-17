/// A listening TCP port from lsof output.
pub(crate) struct ListeningPort {
    pub pid: u32,
    pub port: u16,
    pub address: String,
    pub command: String,
}

/// A listening port matched to a muster session.
pub(crate) struct MatchedPort {
    pub port: u16,
    pub address: String,
    pub pid: u32,
    pub command: String,
    pub session_name: String,
    pub display_name: String,
    pub color: String,
    pub window_index: u32,
    pub window_name: String,
}

/// Parse `lsof -i -P -n -sTCP:LISTEN` output into listening port entries.
pub(crate) fn parse_listening_ports(output: &str) -> Vec<ListeningPort> {
    output
        .lines()
        .skip(1) // skip header
        .filter_map(|line| {
            // Columns: COMMAND PID USER FD TYPE DEVICE SIZE/OFF NODE NAME
            let cols: Vec<&str> = line.split_whitespace().collect();
            if cols.len() < 10 {
                return None;
            }
            let command = cols[0].to_string();
            let pid: u32 = cols[1].parse().ok()?;
            // NAME field is second-to-last: "*:8000 (LISTEN)" splits into
            // [..., "*:8000", "(LISTEN)"]
            let name = cols[cols.len() - 2];
            let (address, port_str) = name.rsplit_once(':')?;
            let port: u16 = port_str.parse().ok()?;
            Some(ListeningPort {
                pid,
                port,
                address: address.to_string(),
                command,
            })
        })
        .collect()
}

/// Run `lsof -i -P -n -sTCP:LISTEN` and parse all listening TCP ports.
/// Returns `None` if lsof is unavailable or fails, `Some(vec)` on success.
pub(crate) fn build_listening_ports() -> Option<Vec<ListeningPort>> {
    let output = match std::process::Command::new("lsof")
        .args(["-i", "-P", "-n", "-sTCP:LISTEN"])
        .output()
    {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).into_owned(),
        // lsof exits with 1 when there are no results — still a successful run
        Ok(o) if o.status.code() == Some(1) => return Some(Vec::new()),
        _ => return None,
    };
    Some(parse_listening_ports(&output))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_lsof_basic() {
        let output = "\
COMMAND     PID USER   FD   TYPE             DEVICE SIZE/OFF NODE NAME
python3.1 12345  usr    4u  IPv4 0xabcdef1234567890      0t0  TCP *:8000 (LISTEN)
node      23456  usr   21u  IPv6 0x1234567890abcdef      0t0  TCP [::1]:5173 (LISTEN)
";
        let ports = parse_listening_ports(output);
        assert_eq!(ports.len(), 2);

        assert_eq!(ports[0].command, "python3.1");
        assert_eq!(ports[0].pid, 12345);
        assert_eq!(ports[0].port, 8000);
        assert_eq!(ports[0].address, "*");

        assert_eq!(ports[1].command, "node");
        assert_eq!(ports[1].pid, 23456);
        assert_eq!(ports[1].port, 5173);
        assert_eq!(ports[1].address, "[::1]");
    }

    #[test]
    fn parse_lsof_localhost() {
        let output = "\
COMMAND   PID USER   FD   TYPE             DEVICE SIZE/OFF NODE NAME
Obsidian 9999  usr   36u  IPv4 0xaabbccdd11223344      0t0  TCP 127.0.0.1:27124 (LISTEN)
";
        let ports = parse_listening_ports(output);
        assert_eq!(ports.len(), 1);
        assert_eq!(ports[0].address, "127.0.0.1");
        assert_eq!(ports[0].port, 27124);
    }

    #[test]
    fn parse_lsof_empty_output() {
        let output = "COMMAND PID USER FD TYPE DEVICE SIZE/OFF NODE NAME\n";
        let ports = parse_listening_ports(output);
        assert!(ports.is_empty());
    }

    #[test]
    fn parse_lsof_skips_short_lines() {
        let output = "\
COMMAND     PID USER   FD   TYPE             DEVICE SIZE/OFF NODE NAME
short line
python3   12345  usr    4u  IPv4 0xabcdef1234567890      0t0  TCP *:9000 (LISTEN)
";
        let ports = parse_listening_ports(output);
        assert_eq!(ports.len(), 1);
        assert_eq!(ports[0].port, 9000);
    }

    #[test]
    fn parse_lsof_ipv6_wildcard() {
        let output = "\
COMMAND     PID USER   FD   TYPE             DEVICE SIZE/OFF NODE NAME
node      11111  usr   19u  IPv6 0xdeadbeef12345678      0t0  TCP *:3000 (LISTEN)
";
        let ports = parse_listening_ports(output);
        assert_eq!(ports.len(), 1);
        assert_eq!(ports[0].address, "*");
        assert_eq!(ports[0].port, 3000);
    }

    #[test]
    fn parse_lsof_multiple_ports_same_process() {
        let output = "\
COMMAND     PID USER   FD   TYPE             DEVICE SIZE/OFF NODE NAME
node      11111  usr   19u  IPv4 0xaaaa000000000001      0t0  TCP *:3000 (LISTEN)
node      11111  usr   20u  IPv6 0xaaaa000000000002      0t0  TCP *:3000 (LISTEN)
node      11111  usr   21u  IPv4 0xaaaa000000000003      0t0  TCP 127.0.0.1:3001 (LISTEN)
";
        let ports = parse_listening_ports(output);
        assert_eq!(ports.len(), 3);
        assert!(ports.iter().all(|p| p.pid == 11111));
        assert_eq!(ports[0].port, 3000);
        assert_eq!(ports[2].port, 3001);
        assert_eq!(ports[2].address, "127.0.0.1");
    }
}
