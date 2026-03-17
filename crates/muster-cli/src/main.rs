//! CLI entry point for muster: terminal session group management built on tmux.

use std::path::PathBuf;
use std::process;

use clap::Parser;
use muster::Muster;
use muster_cli::{Cli, Command, NotificationAction, ProfileAction};

mod commands;
mod editing;
mod format;
mod ports;
mod proctree;
mod resources;
mod tabs;
mod terminal;

fn default_config_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join(".config")
        .join("muster")
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    let config_dir = cli.config_dir.unwrap_or_else(default_config_dir);
    let m = Muster::init(&config_dir)?;
    let settings = m.settings().unwrap_or_default();

    let ctx = commands::CommandContext {
        muster: m,
        settings,
        config_dir,
        json: cli.json,
    };

    match cli.command {
        Command::List => commands::list::execute(&ctx),
        Command::Launch { profile, detach } => commands::launch::execute(&ctx, &profile, detach),
        Command::Attach { session, window } => commands::attach::execute(&ctx, &session, window),
        Command::Kill { session } => commands::kill::execute(&ctx, &session),
        Command::New {
            name,
            tab,
            color,
            detach,
        } => commands::new::execute(&ctx, &name, &tab, &color, detach),
        Command::Color {
            session,
            color,
            list,
        } => commands::color::execute(&ctx, session.as_deref(), color.as_deref(), list),
        Command::Ps { profile } => commands::inspect::execute_ps(&ctx, profile.as_deref()),
        Command::Ports { profile } => commands::inspect::execute_ports(&ctx, profile.as_deref()),
        Command::Top { profile } => commands::inspect::execute_top(&ctx, profile.as_deref()),
        Command::Status => commands::status::execute(&ctx),
        Command::Peek {
            session,
            windows,
            lines,
        } => commands::peek::execute(&ctx, &session, &windows, lines),
        Command::Pin => commands::pin::execute_pin(&ctx),
        Command::Unpin => commands::pin::execute_unpin(&ctx),
        Command::SyncRename {
            session,
            window,
            name,
        } => commands::hooks::execute_sync_rename(&ctx, &session, window, &name),
        Command::PaneDied {
            session_name,
            window_name,
            pane_id,
            exit_code,
        } => commands::hooks::execute_pane_died(
            &ctx,
            &session_name,
            &window_name,
            &pane_id,
            exit_code,
        ),
        Command::Bell {
            session_name,
            window_name,
        } => commands::hooks::execute_bell(&ctx, &session_name, &window_name),
        Command::Notifications { action } => match action {
            NotificationAction::Setup => commands::notifications::execute_setup(),
            NotificationAction::Remove => commands::notifications::execute_remove(),
            NotificationAction::Test => commands::notifications::execute_test(&ctx),
        },
        Command::Profile { action } => match action {
            ProfileAction::List => commands::profile::execute_list(&ctx),
            ProfileAction::Delete { id } => commands::profile::execute_delete(&ctx, &id),
            ProfileAction::Save { name, tab, color } => {
                commands::profile::execute_save(&ctx, &name, &tab, &color)
            }
            ProfileAction::AddTab {
                profile,
                name,
                cwd,
                command,
            } => commands::profile::execute_add_tab(&ctx, &profile, name, cwd, command),
            ProfileAction::Show { id } => commands::profile::execute_show(&ctx, &id),
            ProfileAction::Edit { id } => commands::profile::execute_edit(&ctx, &id),
            ProfileAction::Update { id, name, color } => {
                commands::profile::execute_update(&ctx, &id, name.as_deref(), color.as_deref())
            }
            ProfileAction::RemoveTab { profile, tab } => {
                commands::profile::execute_remove_tab(&ctx, &profile, &tab)
            }
        },
    }
}

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    if let Err(e) = run() {
        eprintln!("Error: {e}");
        process::exit(1);
    }
}
