use std::collections::{BTreeMap, HashMap};

use super::{CommandContext, filter_sessions};
use crate::error::bail;
use crate::format::{color_dot, format_memory};
use crate::ports::{MatchedPort, build_listening_ports};
use crate::proctree::{build_process_table, build_tree, collect_pids, render_tree};
use crate::resources::{build_gpu_table, build_resource_table, collect_tree_resources};

pub(crate) fn execute_ps(ctx: &CommandContext, profile: Option<&str>) -> crate::error::Result {
    let mut sessions = ctx.muster.list_sessions()?;
    filter_sessions(&mut sessions, profile)?;

    if sessions.is_empty() {
        if ctx.json {
            println!("[]");
        } else {
            println!("No active sessions.");
        }
        return Ok(());
    }

    let proc_table = build_process_table();

    if ctx.json {
        let mut json_sessions = Vec::new();
        for s in &sessions {
            let panes = ctx
                .muster
                .client()
                .list_panes(&s.session_name)
                .unwrap_or_default();
            let mut window_map: BTreeMap<u32, Vec<&muster::TmuxPane>> = BTreeMap::new();
            for pane in &panes {
                window_map.entry(pane.window_index).or_default().push(pane);
            }
            let windows = ctx
                .muster
                .client()
                .list_windows(&s.session_name)
                .unwrap_or_default();
            let json_windows: Vec<serde_json::Value> = windows
                .iter()
                .map(|w| {
                    let w_panes = window_map.get(&w.index).cloned().unwrap_or_default();
                    let json_panes: Vec<serde_json::Value> = w_panes
                        .iter()
                        .map(|p| {
                            let children = build_tree(p.pid, &proc_table);
                            serde_json::json!({
                                "index": p.index,
                                "pid": p.pid,
                                "command": p.command,
                                "cwd": p.cwd,
                                "children": children,
                            })
                        })
                        .collect();
                    serde_json::json!({
                        "index": w.index,
                        "name": w.name,
                        "cwd": w.cwd,
                        "panes": json_panes,
                    })
                })
                .collect();

            json_sessions.push(serde_json::json!({
                "session": s.session_name,
                "display_name": s.display_name,
                "color": s.color,
                "windows": json_windows,
            }));
        }
        println!("{}", serde_json::to_string_pretty(&json_sessions)?);
    } else {
        for s in &sessions {
            let panes = ctx
                .muster
                .client()
                .list_panes(&s.session_name)
                .unwrap_or_default();
            let windows = ctx
                .muster
                .client()
                .list_windows(&s.session_name)
                .unwrap_or_default();

            println!(
                "{} {} ({}) [{} windows]",
                color_dot(&s.color),
                s.display_name,
                s.session_name,
                s.window_count,
            );

            let mut pane_map: BTreeMap<u32, Vec<&muster::TmuxPane>> = BTreeMap::new();
            for pane in &panes {
                pane_map.entry(pane.window_index).or_default().push(pane);
            }

            for w in &windows {
                println!("  [{}] {} {}", w.index, w.name, w.cwd);
                if let Some(w_panes) = pane_map.get(&w.index) {
                    for pane in w_panes {
                        println!("      {} (PID {})", pane.command, pane.pid);
                        let children = build_tree(pane.pid, &proc_table);
                        render_tree(&children, "        ");
                    }
                }
            }
        }
    }

    Ok(())
}

#[allow(clippy::too_many_lines)]
pub(crate) fn execute_ports(ctx: &CommandContext, profile: Option<&str>) -> crate::error::Result {
    let mut sessions = ctx.muster.list_sessions()?;
    filter_sessions(&mut sessions, profile)?;

    if sessions.is_empty() {
        if ctx.json {
            println!("[]");
        } else {
            println!("No active sessions.");
        }
        return Ok(());
    }

    let Some(listening) = build_listening_ports() else {
        bail!("Could not query listening ports: lsof not found or failed.");
    };
    if listening.is_empty() {
        if ctx.json {
            println!("[]");
        } else {
            println!("No listening ports found in muster sessions.");
        }
        return Ok(());
    }

    let proc_table = build_process_table();

    // Build a PID -> (session, window_index, window_name) lookup
    let mut pid_lookup: HashMap<u32, (String, String, String, u32, String)> = HashMap::new();

    for s in &sessions {
        let panes = ctx
            .muster
            .client()
            .list_panes(&s.session_name)
            .unwrap_or_default();
        let windows = ctx
            .muster
            .client()
            .list_windows(&s.session_name)
            .unwrap_or_default();

        let window_names: HashMap<u32, String> =
            windows.iter().map(|w| (w.index, w.name.clone())).collect();

        for pane in &panes {
            let tree = build_tree(pane.pid, &proc_table);
            let mut all_pids = vec![pane.pid];
            all_pids.extend(collect_pids(&tree));

            let window_name = window_names
                .get(&pane.window_index)
                .cloned()
                .unwrap_or_default();

            for pid in all_pids {
                pid_lookup.entry(pid).or_insert_with(|| {
                    (
                        s.session_name.clone(),
                        s.display_name.clone(),
                        s.color.clone(),
                        pane.window_index,
                        window_name.clone(),
                    )
                });
            }
        }
    }

    // Match listening ports to sessions
    let mut matched: Vec<MatchedPort> = listening
        .iter()
        .filter_map(|lp| {
            pid_lookup.get(&lp.pid).map(
                |(session_name, display_name, color, window_index, window_name)| MatchedPort {
                    port: lp.port,
                    address: lp.address.clone(),
                    pid: lp.pid,
                    command: lp.command.clone(),
                    session_name: session_name.clone(),
                    display_name: display_name.clone(),
                    color: color.clone(),
                    window_index: *window_index,
                    window_name: window_name.clone(),
                },
            )
        })
        .collect();

    if matched.is_empty() {
        if ctx.json {
            println!("[]");
        } else {
            println!("No listening ports found in muster sessions.");
        }
    } else if ctx.json {
        let json_ports: Vec<serde_json::Value> = matched
            .iter()
            .map(|mp| {
                serde_json::json!({
                    "port": mp.port,
                    "address": mp.address,
                    "pid": mp.pid,
                    "command": mp.command,
                    "session": mp.session_name,
                    "display_name": mp.display_name,
                    "color": mp.color,
                    "window_index": mp.window_index,
                    "window_name": mp.window_name,
                })
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&json_ports)?);
    } else {
        matched.sort_by(|a, b| {
            a.session_name
                .cmp(&b.session_name)
                .then(a.port.cmp(&b.port))
        });

        let mut current_session = String::new();
        for mp in &matched {
            if mp.session_name != current_session {
                if !current_session.is_empty() {
                    println!();
                }
                println!(
                    "{} {} ({})",
                    color_dot(&mp.color),
                    mp.display_name,
                    mp.session_name,
                );
                current_session.clone_from(&mp.session_name);
            }
            println!(
                "  :{:<6} {:<16} [{}] {}",
                mp.port, mp.command, mp.window_index, mp.window_name,
            );
        }
    }

    Ok(())
}

#[allow(clippy::too_many_lines)]
pub(crate) fn execute_top(ctx: &CommandContext, profile: Option<&str>) -> crate::error::Result {
    let mut sessions = ctx.muster.list_sessions()?;
    filter_sessions(&mut sessions, profile)?;

    if sessions.is_empty() {
        if ctx.json {
            println!("[]");
        } else {
            println!("No active sessions.");
        }
        return Ok(());
    }

    let res_table = build_resource_table();
    let gpu_table = build_gpu_table();
    let has_gpu = gpu_table.is_some();
    let gpu_entries = gpu_table.unwrap_or_default();

    let gpu_lookup: HashMap<u32, u64> = gpu_entries
        .iter()
        .map(|g| (g.pid, g.gpu_memory_mb))
        .collect();

    if ctx.json {
        let mut json_sessions = Vec::new();
        for s in &sessions {
            let panes = ctx
                .muster
                .client()
                .list_panes(&s.session_name)
                .unwrap_or_default();
            let windows = ctx
                .muster
                .client()
                .list_windows(&s.session_name)
                .unwrap_or_default();

            let mut session_cpu = 0.0;
            let mut session_rss = 0u64;
            let mut session_gpu = 0u64;

            let mut pane_map: BTreeMap<u32, Vec<&muster::TmuxPane>> = BTreeMap::new();
            for pane in &panes {
                pane_map.entry(pane.window_index).or_default().push(pane);
            }

            let mut json_windows = Vec::new();
            for w in &windows {
                let mut win_cpu = 0.0;
                let mut win_rss = 0u64;
                let mut win_gpu = 0u64;

                if let Some(w_panes) = pane_map.get(&w.index) {
                    for pane in w_panes {
                        let (cpu, rss) = collect_tree_resources(pane.pid, &res_table);
                        win_cpu += cpu;
                        win_rss += rss;
                        let proc_table = build_tree(pane.pid, &build_process_table());
                        let mut all_pids = vec![pane.pid];
                        all_pids.extend(collect_pids(&proc_table));
                        for pid in &all_pids {
                            if let Some(&mem) = gpu_lookup.get(pid) {
                                win_gpu += mem;
                            }
                        }
                    }
                }

                session_cpu += win_cpu;
                session_rss += win_rss;
                session_gpu += win_gpu;

                let mut win_json = serde_json::json!({
                    "index": w.index,
                    "name": w.name,
                    "cpu_percent": (win_cpu * 10.0).round() / 10.0,
                    "rss_kb": win_rss,
                });
                if has_gpu {
                    win_json["gpu_memory_mb"] = serde_json::json!(win_gpu);
                }
                json_windows.push(win_json);
            }

            let mut sess_json = serde_json::json!({
                "session": s.session_name,
                "display_name": s.display_name,
                "color": s.color,
                "cpu_percent": (session_cpu * 10.0).round() / 10.0,
                "rss_kb": session_rss,
                "windows": json_windows,
            });
            if has_gpu {
                sess_json["gpu_memory_mb"] = serde_json::json!(session_gpu);
            }
            json_sessions.push(sess_json);
        }
        println!("{}", serde_json::to_string_pretty(&json_sessions)?);
    } else {
        struct WindowStats {
            index: u32,
            name: String,
            cpu: f64,
            rss: u64,
            gpu: u64,
        }

        let mut total_cpu = 0.0;
        let mut total_rss = 0u64;
        let mut total_gpu = 0u64;

        for s in &sessions {
            let panes = ctx
                .muster
                .client()
                .list_panes(&s.session_name)
                .unwrap_or_default();
            let windows = ctx
                .muster
                .client()
                .list_windows(&s.session_name)
                .unwrap_or_default();

            let mut session_cpu = 0.0;
            let mut session_rss = 0u64;
            let mut session_gpu = 0u64;

            let mut pane_map: BTreeMap<u32, Vec<&muster::TmuxPane>> = BTreeMap::new();
            for pane in &panes {
                pane_map.entry(pane.window_index).or_default().push(pane);
            }

            let proc_table_for_gpu = build_process_table();
            let mut window_stats = Vec::new();
            for w in &windows {
                let mut win_cpu = 0.0;
                let mut win_rss = 0u64;
                let mut win_gpu = 0u64;

                if let Some(w_panes) = pane_map.get(&w.index) {
                    for pane in w_panes {
                        let (cpu, rss) = collect_tree_resources(pane.pid, &res_table);
                        win_cpu += cpu;
                        win_rss += rss;
                        let tree = build_tree(pane.pid, &proc_table_for_gpu);
                        let mut all_pids = vec![pane.pid];
                        all_pids.extend(collect_pids(&tree));
                        for pid in &all_pids {
                            if let Some(&mem) = gpu_lookup.get(pid) {
                                win_gpu += mem;
                            }
                        }
                    }
                }

                session_cpu += win_cpu;
                session_rss += win_rss;
                session_gpu += win_gpu;

                window_stats.push(WindowStats {
                    index: w.index,
                    name: w.name.clone(),
                    cpu: win_cpu,
                    rss: win_rss,
                    gpu: win_gpu,
                });
            }

            total_cpu += session_cpu;
            total_rss += session_rss;
            total_gpu += session_gpu;

            let gpu_str = if has_gpu {
                format!("  GPU: {session_gpu} MB")
            } else {
                String::new()
            };
            println!(
                "{} {} ({})  CPU: {:.1}%  Mem: {}{}",
                color_dot(&s.color),
                s.display_name,
                s.session_name,
                session_cpu,
                format_memory(session_rss),
                gpu_str,
            );

            for ws in &window_stats {
                if ws.cpu < 0.1 && ws.rss < 1024 && ws.gpu == 0 {
                    continue;
                }
                let win_gpu_str = if has_gpu && ws.gpu > 0 {
                    format!("  GPU: {} MB", ws.gpu)
                } else {
                    String::new()
                };
                println!(
                    "  [{}] {:<20} CPU: {:>5.1}%  Mem: {:>10}{}",
                    ws.index,
                    ws.name,
                    ws.cpu,
                    format_memory(ws.rss),
                    win_gpu_str,
                );
            }
        }

        if sessions.len() > 1 {
            let gpu_str = if has_gpu {
                format!("  GPU: {total_gpu} MB")
            } else {
                String::new()
            };
            println!(
                "\nTotal: CPU: {:.1}%  Mem: {}{}",
                total_cpu,
                format_memory(total_rss),
                gpu_str,
            );
        }
    }

    Ok(())
}
