//! `slate-dashboard` — widgets and the dashboard engine.
//!
//! A [`Widget`] is a tiny state machine: `refresh(env)` samples the world on
//! its own cadence ([`Widget::interval`]), `render(width, theme)` turns the
//! sampled state into styled [`Line`]s. The [`Dashboard`] drives a set of
//! widgets (configured via `dashboard.widgets`).

#![forbid(unsafe_code)]

pub mod probe;
pub mod widgets;

use std::path::Path;

use slate_core::civil::DateTime;
use slate_core::style::Line;
use slate_theme::Theme;

pub use probe::Probe;
pub use widgets::{all_widgets, widget_by_name};

/// Everything a widget may sample from.
pub struct WidgetEnv<'a> {
    pub probe: &'a mut Probe,
    pub cwd: &'a Path,
    pub workspace: &'a str,
    pub notifications: usize,
    pub weather_location: Option<&'a str>,
    /// Demo mode: deterministic fake data (used by `slate snapshot`).
    pub demo: bool,
}

impl WidgetEnv<'_> {
    /// Every dependency replaced by deterministic values — used for golden
    /// snapshot tests and README screenshots.
    pub fn demo(probe: &mut Probe) -> WidgetEnv<'_> {
        WidgetEnv {
            probe,
            cwd: Path::new("."),
            workspace: "main",
            notifications: 3,
            weather_location: Some("Berlin"),
            demo: true,
        }
    }
}

/// A dashboard widget.
pub trait Widget {
    /// Configuration/registry name (`dashboard.widgets = ["cpu"]`).
    fn name(&self) -> &'static str;
    /// One-line description for `dashboard widgets`.
    fn about(&self) -> &'static str;
    /// Seconds between refreshes (0 = every tick).
    fn interval(&self) -> u64;
    /// Sample the world.
    fn refresh(&mut self, env: &mut WidgetEnv<'_>);
    /// Produce display lines.
    fn render(&self, width: u16, theme: &Theme) -> Vec<Line>;
}

/// An active dashboard.
pub struct Dashboard {
    widgets: Vec<ActiveWidget>,
}

struct ActiveWidget {
    widget: Box<dyn Widget>,
    last_refresh: i64,
}

impl Dashboard {
    /// Build from configured widget names (unknown names are skipped).
    pub fn new(names: &[String]) -> Self {
        let widgets = names
            .iter()
            .filter_map(|n| widget_by_name(n))
            .map(|widget| ActiveWidget { widget, last_refresh: 0 })
            .collect();
        Dashboard { widgets }
    }

    pub fn names(&self) -> Vec<&'static str> {
        self.widgets.iter().map(|w| w.widget.name()).collect()
    }

    pub fn len(&self) -> usize {
        self.widgets.len()
    }

    pub fn is_empty(&self) -> bool {
        self.widgets.is_empty()
    }

    /// Refresh widgets whose interval has elapsed. Returns how many did.
    pub fn refresh(&mut self, env: &mut WidgetEnv<'_>) -> usize {
        let now = DateTime::now_utc().secs;
        let mut count = 0;
        for active in &mut self.widgets {
            let interval = active.widget.interval() as i64;
            if interval == 0 || now - active.last_refresh >= interval || active.last_refresh == 0 {
                active.widget.refresh(env);
                active.last_refresh = now;
                count += 1;
            }
        }
        count
    }

    /// Refresh everything regardless of interval (`dashboard` command).
    pub fn force_refresh(&mut self, env: &mut WidgetEnv<'_>) {
        let now = DateTime::now_utc().secs;
        for active in &mut self.widgets {
            active.widget.refresh(env);
            active.last_refresh = now;
        }
    }

    /// Concatenate all widget render lines.
    pub fn render(&self, width: u16, theme: &Theme) -> Vec<Line> {
        let mut out = Vec::new();
        for (i, active) in self.widgets.iter().enumerate() {
            if i > 0 {
                out.push(Line::default()); // blank spacer between widgets
            }
            out.extend(active.widget.render(width, theme));
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use slate_theme::ThemeManager;

    fn names(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn builds_from_config_names() {
        let d = Dashboard::new(&names(&["clock", "cpu", "does-not-exist"]));
        assert_eq!(d.names(), vec!["clock", "cpu"]);
        assert_eq!(d.len(), 2);
    }

    #[test]
    fn demo_refresh_and_render() {
        let mut probe = Probe::new();
        let mut d = Dashboard::new(&names(&["clock", "cpu", "memory", "battery"]));
        let mut env = WidgetEnv::demo(&mut probe);
        let refreshed = d.refresh(&mut env);
        assert!(refreshed >= 3);
        let tm = ThemeManager::builtins();
        let lines = d.render(40, tm.current());
        assert!(lines.len() >= 4);
        // Demo clock renders deterministic content.
        let text: String = lines.iter().map(Line::to_plain).collect::<Vec<_>>().join("\n");
        assert!(text.contains("09:41"), "{text}");
        assert!(text.contains('%'), "{text}");
    }

    #[test]
    fn widget_registry_lists_all() {
        let all = all_widgets();
        assert!(all.iter().any(|(n, _)| *n == "clock"));
        assert!(all.iter().any(|(n, _)| *n == "git"));
        for (name, _about) in all {
            assert!(widget_by_name(name).is_some(), "widget '{name}' missing factory");
        }
        assert!(widget_by_name("nope").is_none());
    }
}
