use muster::{Profile, TabProfile};

/// TOML representation of a profile for interactive editing.
/// Excludes `id` since it's derived from `name` via slugify.
#[derive(serde::Serialize, serde::Deserialize)]
pub(crate) struct EditableProfile {
    pub name: String,
    pub color: String,
    pub tabs: Vec<EditableTab>,
}

#[derive(serde::Serialize, serde::Deserialize)]
pub(crate) struct EditableTab {
    pub name: String,
    pub cwd: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layout: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub panes: Vec<EditablePane>,
}

#[derive(serde::Serialize, serde::Deserialize)]
pub(crate) struct EditablePane {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
}

impl From<&Profile> for EditableProfile {
    fn from(p: &Profile) -> Self {
        Self {
            name: p.name.clone(),
            color: p.color.clone(),
            tabs: p
                .tabs
                .iter()
                .map(|t| EditableTab {
                    name: t.name.clone(),
                    cwd: t.cwd.clone(),
                    command: t.command.clone(),
                    layout: t.layout.clone(),
                    panes: t
                        .panes
                        .iter()
                        .map(|p| EditablePane {
                            cwd: p.cwd.clone(),
                            command: p.command.clone(),
                        })
                        .collect(),
                })
                .collect(),
        }
    }
}

impl EditableProfile {
    pub fn into_profile(self) -> Profile {
        Profile {
            id: muster::config::profile::slugify(&self.name),
            name: self.name,
            color: self.color,
            tabs: self
                .tabs
                .into_iter()
                .map(|t| TabProfile {
                    name: t.name,
                    cwd: t.cwd,
                    command: t.command,
                    layout: t.layout,
                    panes: t
                        .panes
                        .into_iter()
                        .map(|p| muster::PaneProfile {
                            cwd: p.cwd,
                            command: p.command,
                        })
                        .collect(),
                })
                .collect(),
        }
    }
}
