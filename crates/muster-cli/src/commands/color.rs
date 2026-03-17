use std::io::IsTerminal;
use std::process;

use super::CommandContext;

pub(crate) fn execute(
    ctx: &CommandContext,
    session: Option<&str>,
    color: Option<&str>,
    list: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    if list {
        if ctx.json {
            let colors: Vec<serde_json::Value> = muster::NAMED_COLORS
                .iter()
                .map(|(name, aliases, hex)| {
                    serde_json::json!({
                        "name": name,
                        "aliases": aliases,
                        "hex": hex,
                    })
                })
                .collect();
            let shades: Vec<serde_json::Value> = muster::TAILWIND_SHADES
                .iter()
                .map(|(name, light, base, dark)| {
                    serde_json::json!({
                        "name": name,
                        "light": light,
                        "base": base,
                        "dark": dark,
                    })
                })
                .collect();
            let output = serde_json::json!({
                "colors": colors,
                "shades": shades,
            });
            println!("{}", serde_json::to_string_pretty(&output)?);
        } else {
            fn hex_to_rgb(hex: &str) -> (u8, u8, u8) {
                let h = hex.strip_prefix('#').unwrap_or(hex);
                let r = u8::from_str_radix(&h[0..2], 16).unwrap_or(0);
                let g = u8::from_str_radix(&h[2..4], 16).unwrap_or(0);
                let b = u8::from_str_radix(&h[4..6], 16).unwrap_or(0);
                (r, g, b)
            }
            let is_tty = std::io::stdout().is_terminal();
            for (name, aliases, hex) in muster::NAMED_COLORS {
                let (r, g, b) = hex_to_rgb(hex);
                let alias_str = if aliases.is_empty() {
                    String::new()
                } else {
                    format!(" ({})", aliases.join(", "))
                };
                if is_tty {
                    println!("\x1b[48;2;{r};{g};{b}m  \x1b[0m  {name}{alias_str}  {hex}",);
                } else {
                    println!("{name}{alias_str}  {hex}");
                }
            }
            println!();
            println!("Shades: append -light or -dark (e.g. red-light, blue-dark)");
        }
    } else {
        let (Some(session), Some(color)) = (session, color) else {
            eprintln!("Usage: muster color <session> <color>");
            eprintln!("       muster color --list");
            process::exit(1);
        };
        if let Ok(session_name) = ctx.muster.resolve_session(session) {
            ctx.muster.set_color(&session_name, color)?;
            if !ctx.json {
                println!("Color updated: {session_name} → {color}");
            }
        } else {
            // No running session — try updating the profile directly
            let profiles = ctx.muster.list_profiles()?;
            let found = profiles
                .iter()
                .find(|p| p.name == session || p.id == session);
            let Some(p) = found else {
                eprintln!("No session or profile found: {session}");
                process::exit(1);
            };
            let resolved = muster::session::theme::resolve_color(color)?;
            let mut profile = p.clone();
            profile.color = resolved;
            ctx.muster.update_profile(profile)?;
            if !ctx.json {
                println!("Color updated: {} → {color}", p.name);
            }
        }
    }

    Ok(())
}
