/// Information about a single process from the system process table.
pub(crate) struct ProcessInfo {
    pub pid: u32,
    pub ppid: u32,
    pub command: String,
}

/// Recursive tree of processes, rooted at a given PID.
#[derive(serde::Serialize)]
pub(crate) struct ProcessTree {
    pub pid: u32,
    pub command: String,
    pub children: Vec<ProcessTree>,
}

/// Parse `ps -eo pid,ppid,comm` output into a process table.
pub(crate) fn parse_process_table(output: &str) -> Vec<ProcessInfo> {
    output
        .lines()
        .skip(1) // skip header
        .filter_map(|line| {
            let line = line.trim();
            let mut tokens = line.split_whitespace();
            let pid: u32 = tokens.next()?.parse().ok()?;
            let parent: u32 = tokens.next()?.parse().ok()?;
            // Rejoin the rest — command may contain spaces
            let command: String = tokens.collect::<Vec<_>>().join(" ");
            if command.is_empty() {
                return None;
            }
            Some(ProcessInfo {
                pid,
                ppid: parent,
                command,
            })
        })
        .collect()
}

/// Run `ps -eo pid,ppid,comm` and parse the full process table.
pub(crate) fn build_process_table() -> Vec<ProcessInfo> {
    let output = match std::process::Command::new("ps")
        .args(["-eo", "pid,ppid,comm"])
        .output()
    {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).into_owned(),
        _ => return Vec::new(),
    };
    parse_process_table(&output)
}

/// Build a process tree rooted at `root_pid` from the system process table.
pub(crate) fn build_tree(root_pid: u32, table: &[ProcessInfo]) -> Vec<ProcessTree> {
    let children: Vec<&ProcessInfo> = table.iter().filter(|p| p.ppid == root_pid).collect();
    children
        .into_iter()
        .map(|child| ProcessTree {
            pid: child.pid,
            command: child.command.clone(),
            children: build_tree(child.pid, table),
        })
        .collect()
}

/// Render a process tree with box-drawing characters at a given indent level.
pub(crate) fn render_tree(tree: &[ProcessTree], prefix: &str) {
    for (i, node) in tree.iter().enumerate() {
        let is_last = i == tree.len() - 1;
        let connector = if is_last { "└─" } else { "├─" };
        println!("{prefix}{connector} {} (PID {})", node.command, node.pid);
        let child_prefix = if is_last {
            format!("{prefix}   ")
        } else {
            format!("{prefix}│  ")
        };
        render_tree(&node.children, &child_prefix);
    }
}

/// Recursively collect all PIDs from a process tree.
pub(crate) fn collect_pids(tree: &[ProcessTree]) -> Vec<u32> {
    let mut pids = Vec::new();
    for node in tree {
        pids.push(node.pid);
        pids.extend(collect_pids(&node.children));
    }
    pids
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_ps_basic() {
        let output = "  PID  PPID COMM\n    1     0 /sbin/launchd\n  100     1 /usr/sbin/syslogd\n  200   100 /usr/bin/some_daemon\n";
        let table = parse_process_table(output);
        assert_eq!(table.len(), 3);
        assert_eq!(table[0].pid, 1);
        assert_eq!(table[0].ppid, 0);
        assert_eq!(table[0].command, "/sbin/launchd");
        assert_eq!(table[1].pid, 100);
        assert_eq!(table[1].ppid, 1);
        assert_eq!(table[2].pid, 200);
        assert_eq!(table[2].ppid, 100);
    }

    #[test]
    fn parse_ps_empty_output() {
        let output = "  PID  PPID COMM\n";
        let table = parse_process_table(output);
        assert!(table.is_empty());
    }

    #[test]
    fn parse_ps_command_with_spaces() {
        let output = "  PID  PPID COMM\n  500   100 /usr/local/bin/my tool\n";
        let table = parse_process_table(output);
        assert_eq!(table.len(), 1);
        assert_eq!(table[0].command, "/usr/local/bin/my tool");
    }

    #[test]
    fn parse_ps_skips_malformed_lines() {
        let output = "  PID  PPID COMM\n  notapid  1 /bin/sh\n  100     1 /usr/bin/daemon\n  abc   def ghi\n";
        let table = parse_process_table(output);
        assert_eq!(table.len(), 1);
        assert_eq!(table[0].pid, 100);
    }

    fn sample_process_table() -> Vec<ProcessInfo> {
        vec![
            ProcessInfo {
                pid: 10,
                ppid: 1,
                command: "fish".into(),
            },
            ProcessInfo {
                pid: 20,
                ppid: 1,
                command: "bash".into(),
            },
            ProcessInfo {
                pid: 100,
                ppid: 10,
                command: "npm".into(),
            },
            ProcessInfo {
                pid: 101,
                ppid: 100,
                command: "node".into(),
            },
            ProcessInfo {
                pid: 102,
                ppid: 10,
                command: "cargo".into(),
            },
        ]
    }

    #[test]
    fn build_tree_from_root() {
        let table = sample_process_table();
        let tree = build_tree(1, &table);
        assert_eq!(tree.len(), 2);
        assert_eq!(tree[0].command, "fish");
        assert_eq!(tree[0].children.len(), 2);
        assert_eq!(tree[1].command, "bash");
        assert!(tree[1].children.is_empty());
    }

    #[test]
    fn build_tree_from_subtree() {
        let table = sample_process_table();
        let tree = build_tree(10, &table);
        assert_eq!(tree.len(), 2);
        let npm = &tree[0];
        assert_eq!(npm.command, "npm");
        assert_eq!(npm.children.len(), 1);
        assert_eq!(npm.children[0].command, "node");
    }

    #[test]
    fn build_tree_leaf_node() {
        let table = sample_process_table();
        let tree = build_tree(101, &table);
        assert!(tree.is_empty());
    }

    #[test]
    fn build_tree_nonexistent_root() {
        let table = sample_process_table();
        let tree = build_tree(9999, &table);
        assert!(tree.is_empty());
    }

    #[test]
    fn collect_pids_full_tree() {
        let table = sample_process_table();
        let tree = build_tree(1, &table);
        let mut pids = collect_pids(&tree);
        pids.sort();
        assert_eq!(pids, vec![10, 20, 100, 101, 102]);
    }

    #[test]
    fn collect_pids_subtree() {
        let table = sample_process_table();
        let tree = build_tree(10, &table);
        let mut pids = collect_pids(&tree);
        pids.sort();
        assert_eq!(pids, vec![100, 101, 102]);
    }

    #[test]
    fn collect_pids_empty_tree() {
        let pids = collect_pids(&[]);
        assert!(pids.is_empty());
    }
}
