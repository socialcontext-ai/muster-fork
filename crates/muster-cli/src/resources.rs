/// Resource usage entry from `ps` output.
pub(crate) struct ResourceEntry {
    pub pid: u32,
    pub ppid: u32,
    pub cpu: f64,
    pub rss_kb: u64,
    #[allow(dead_code)]
    pub command: String,
}

/// GPU process info from nvidia-smi.
pub(crate) struct GpuProcessInfo {
    pub pid: u32,
    pub gpu_memory_mb: u64,
}

/// Parse `ps -eo pid,ppid,%cpu,rss,comm` output into resource entries.
#[allow(clippy::similar_names)]
pub(crate) fn parse_resource_table(output: &str) -> Vec<ResourceEntry> {
    output
        .lines()
        .skip(1) // skip header
        .filter_map(|line| {
            let line = line.trim();
            let mut tokens = line.split_whitespace();
            let pid: u32 = tokens.next()?.parse().ok()?;
            let ppid: u32 = tokens.next()?.parse().ok()?;
            let cpu: f64 = tokens.next()?.parse().ok()?;
            let rss_kb: u64 = tokens.next()?.parse().ok()?;
            let command: String = tokens.collect::<Vec<_>>().join(" ");
            if command.is_empty() {
                return None;
            }
            Some(ResourceEntry {
                pid,
                ppid,
                cpu,
                rss_kb,
                command,
            })
        })
        .collect()
}

/// Run `ps -eo pid,ppid,%cpu,rss,comm` and parse the full resource table.
pub(crate) fn build_resource_table() -> Vec<ResourceEntry> {
    let output = match std::process::Command::new("ps")
        .args(["-eo", "pid,ppid,%cpu,rss,comm"])
        .output()
    {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).into_owned(),
        _ => return Vec::new(),
    };
    parse_resource_table(&output)
}

/// Collect resource usage for a PID and all its descendants.
pub(crate) fn collect_tree_resources(root_pid: u32, table: &[ResourceEntry]) -> (f64, u64) {
    let mut cpu = 0.0;
    let mut rss = 0u64;
    if let Some(entry) = table.iter().find(|e| e.pid == root_pid) {
        cpu += entry.cpu;
        rss += entry.rss_kb;
    }
    for child in table.iter().filter(|e| e.ppid == root_pid) {
        let (c, r) = collect_tree_resources(child.pid, table);
        cpu += c;
        rss += r;
    }
    (cpu, rss)
}

/// Try to query per-process GPU usage via nvidia-smi.
/// Returns None if nvidia-smi is not available, Some(vec) otherwise.
pub(crate) fn build_gpu_table() -> Option<Vec<GpuProcessInfo>> {
    let output = std::process::Command::new("nvidia-smi")
        .args([
            "--query-compute-apps=pid,used_memory",
            "--format=csv,noheader,nounits",
        ])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let entries = text
        .lines()
        .filter_map(|line| {
            let mut parts = line.split(',').map(str::trim);
            let pid: u32 = parts.next()?.parse().ok()?;
            let mem: u64 = parts.next()?.parse().ok()?;
            Some(GpuProcessInfo {
                pid,
                gpu_memory_mb: mem,
            })
        })
        .collect();
    Some(entries)
}
