mod install;
mod remove;

use std::io::{self, Write};
use std::path::Path;
use std::process::ExitCode;

use orchester_verzeichnis::{PluginOrigin, RegisteredPlugin, Registry};

use crate::args::PluginCommand;

/// Whether a plugin command succeeded.
///
/// [`ExitCode`] is deliberately opaque and cannot be compared, so an
/// interactive caller that wants to keep the session open has no way to notice
/// a failure it did not propagate. Returning an inspectable outcome lets the
/// prompt loops continue *and* still report the failure through the process
/// exit status.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PluginOutcome {
    Succeeded,
    Failed,
}

impl PluginOutcome {
    pub fn failed(self) -> bool {
        self == Self::Failed
    }
}

impl From<PluginOutcome> for ExitCode {
    fn from(outcome: PluginOutcome) -> Self {
        match outcome {
            PluginOutcome::Succeeded => ExitCode::SUCCESS,
            PluginOutcome::Failed => ExitCode::FAILURE,
        }
    }
}

pub fn run(
    registry: &Registry,
    command: PluginCommand,
    json: bool,
    orchester_home: &Path,
) -> io::Result<PluginOutcome> {
    match command {
        PluginCommand::List => {
            render_list(&mut io::stdout().lock(), &registry.plugins(), json)?;
            Ok(PluginOutcome::Succeeded)
        }
        PluginCommand::Status(args) => {
            let plugins = registry.plugins();
            let Some(plugin) = plugins
                .iter()
                .find(|plugin| plugin.info().name() == args.name)
            else {
                writeln!(
                    io::stderr().lock(),
                    "orchester: agent plugin is not installed"
                )?;
                return Ok(PluginOutcome::Failed);
            };
            render_status(&mut io::stdout().lock(), plugin, json)?;
            Ok(PluginOutcome::Succeeded)
        }
        PluginCommand::Install(args) => match install::install(orchester_home, &args.name) {
            Ok(info) => {
                if json {
                    writeln!(
                        io::stdout().lock(),
                        "{}",
                        serde_json::json!({
                            "name": info.name(),
                            "displayName": info.display_name(),
                            "packageName": info.package_name(),
                            "version": info.version(),
                            "origin": "managed",
                        })
                    )?;
                } else {
                    writeln!(
                        io::stdout().lock(),
                        "Installed {} {} ({})",
                        info.display_name(),
                        info.version(),
                        info.package_name()
                    )?;
                }
                Ok(PluginOutcome::Succeeded)
            }
            Err(error) => {
                writeln!(io::stderr().lock(), "orchester: {error}")?;
                Ok(PluginOutcome::Failed)
            }
        },
        PluginCommand::Remove(args) => match remove::remove(orchester_home, &args.name) {
            Ok(remove::RemoveOutcome::Removed(info)) => {
                if json {
                    writeln!(
                        io::stdout().lock(),
                        "{}",
                        serde_json::json!({
                            "name": info.name(),
                            "displayName": info.display_name(),
                            "packageName": info.package_name(),
                            "version": info.version(),
                            "removed": true,
                        })
                    )?;
                } else {
                    writeln!(
                        io::stdout().lock(),
                        "Removed {} {} ({})",
                        info.display_name(),
                        info.version(),
                        info.package_name()
                    )?;
                }
                Ok(PluginOutcome::Succeeded)
            }
            Ok(remove::RemoveOutcome::NotInstalled) => {
                if json {
                    writeln!(
                        io::stdout().lock(),
                        "{}",
                        serde_json::json!({"name": args.name, "removed": false})
                    )?;
                } else {
                    writeln!(io::stdout().lock(), "Plugin is not installed")?;
                }
                Ok(PluginOutcome::Succeeded)
            }
            Err(error) => {
                writeln!(io::stderr().lock(), "orchester: {error}")?;
                Ok(PluginOutcome::Failed)
            }
        },
    }
}

fn render_list(out: &mut impl Write, plugins: &[RegisteredPlugin], json: bool) -> io::Result<()> {
    if plugins.is_empty() {
        return if json {
            Ok(())
        } else {
            writeln!(out, "no agent plugins installed")
        };
    }
    for plugin in plugins {
        if json {
            writeln!(out, "{}", json_value(plugin))?;
        } else {
            let info = plugin.info();
            writeln!(
                out,
                "{}\t{}\t{}\t{}\t{}",
                info.name(),
                info.display_name(),
                info.package_name(),
                info.version(),
                origin_word(plugin.origin())
            )?;
        }
    }
    Ok(())
}

fn render_status(out: &mut impl Write, plugin: &RegisteredPlugin, json: bool) -> io::Result<()> {
    let info = plugin.info();
    if json {
        return writeln!(out, "{}", json_value(plugin));
    }
    writeln!(out, "name: {}", info.name())?;
    writeln!(out, "display: {}", info.display_name())?;
    writeln!(out, "package: {}", info.package_name())?;
    writeln!(out, "version: {}", info.version())?;
    writeln!(out, "origin: {}", origin_word(plugin.origin()))
}

fn json_value(plugin: &RegisteredPlugin) -> serde_json::Value {
    let info = plugin.info();
    serde_json::json!({
        "name": info.name(),
        "displayName": info.display_name(),
        "packageName": info.package_name(),
        "version": info.version(),
        "origin": origin_word(plugin.origin()),
    })
}

fn origin_word(origin: PluginOrigin) -> &'static str {
    match origin {
        PluginOrigin::Managed => "managed",
        PluginOrigin::Project => "project",
    }
}
