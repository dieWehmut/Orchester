use std::env;
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};

use super::CommandInvocation;

pub fn command_invocation(executable: &Path, extra_args: Vec<OsString>) -> CommandInvocation {
    if is_shell_script(executable) {
        if let Some(invocation) = cmd_shim_invocation(executable, &extra_args) {
            return invocation;
        }
        if let Some(ps1) = adjacent_powershell_shim(executable) {
            return powershell_invocation(&ps1, extra_args);
        }
        let mut args = vec![
            OsString::from("/d"),
            OsString::from("/c"),
            executable.as_os_str().to_os_string(),
        ];
        args.extend(extra_args);
        return CommandInvocation {
            program: PathBuf::from("cmd.exe"),
            args,
            envs: Vec::new(),
            shell_backed: true,
        };
    }

    CommandInvocation {
        program: executable.to_path_buf(),
        args: extra_args,
        envs: Vec::new(),
        shell_backed: false,
    }
}

pub(super) fn executable_names(command: &str) -> Vec<OsString> {
    if Path::new(command).extension().is_some() {
        return vec![OsString::from(command)];
    }

    let mut names = Vec::new();
    if let Some(pathext) = env::var_os("PATHEXT") {
        for ext in env::split_paths(&pathext) {
            if let Some(ext) = ext.to_str() {
                names.push(OsString::from(format!("{command}{ext}")));
            }
        }
    } else {
        for ext in [".COM", ".EXE", ".BAT", ".CMD"] {
            names.push(OsString::from(format!("{command}{ext}")));
        }
    }
    names.push(OsString::from(command));
    names
}

fn is_shell_script(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.eq_ignore_ascii_case("cmd") || ext.eq_ignore_ascii_case("bat"))
        .unwrap_or(false)
}

fn adjacent_powershell_shim(executable: &Path) -> Option<PathBuf> {
    let mut ps1 = executable.to_path_buf();
    ps1.set_extension("ps1");
    ps1.is_file().then_some(ps1)
}

fn powershell_invocation(script: &Path, extra_args: Vec<OsString>) -> CommandInvocation {
    let mut args = vec![
        OsString::from("-NoProfile"),
        OsString::from("-ExecutionPolicy"),
        OsString::from("Bypass"),
        OsString::from("-File"),
        script.as_os_str().to_os_string(),
    ];
    args.extend(extra_args);
    CommandInvocation {
        program: PathBuf::from("powershell.exe"),
        args,
        envs: Vec::new(),
        shell_backed: true,
    }
}

fn cmd_shim_invocation(executable: &Path, extra_args: &[OsString]) -> Option<CommandInvocation> {
    let content = fs::read_to_string(executable).ok()?;
    let dir = executable.parent()?;

    if let Some(invocation) = opencode_invocation(&content, dir, extra_args) {
        return Some(invocation);
    }

    let script = find_node_script_entry(&content, dir)?;
    let program = local_node_or_path_node(dir);
    let mut args = vec![script.into_os_string()];
    args.extend(extra_args.iter().cloned());
    Some(CommandInvocation {
        program,
        args,
        envs: node_path_env(&content),
        shell_backed: false,
    })
}

fn opencode_invocation(
    content: &str,
    dir: &Path,
    extra_args: &[OsString],
) -> Option<CommandInvocation> {
    let exe = content
        .lines()
        .find_map(|line| parse_cmd_set(line, "opencode_exe"))
        .map(|value| expand_cmd_path(&value, dir))?;
    if !exe.is_file() {
        return None;
    }

    let mut args = Vec::new();
    if extra_args.is_empty() {
        args.extend(
            ["web", "--hostname", "127.0.0.1", "--port", "4096"]
                .into_iter()
                .map(OsString::from),
        );
    } else {
        args.extend(extra_args.iter().cloned());
    }

    Some(CommandInvocation {
        program: exe,
        args,
        envs: Vec::new(),
        shell_backed: false,
    })
}

fn find_node_script_entry(content: &str, dir: &Path) -> Option<PathBuf> {
    content.lines().find_map(|line| {
        if !line.to_ascii_lowercase().contains(".js") {
            return None;
        }
        quoted_tokens(line).into_iter().find_map(|token| {
            if !token.to_ascii_lowercase().contains(".js") {
                return None;
            }
            let path = expand_cmd_path(&token, dir);
            path.is_file().then_some(path)
        })
    })
}

fn local_node_or_path_node(dir: &Path) -> PathBuf {
    let local = dir.join("node.exe");
    if local.is_file() {
        local
    } else {
        PathBuf::from("node")
    }
}

fn node_path_env(content: &str) -> Vec<(OsString, OsString)> {
    let Some(new_path) = content.lines().find_map(|line| {
        let value = parse_cmd_set(line, "NODE_PATH")?;
        (!value.contains("%NODE_PATH%")).then_some(value)
    }) else {
        return Vec::new();
    };

    let value = match env::var_os("NODE_PATH") {
        Some(existing) if !existing.is_empty() => {
            OsString::from(format!("{new_path};{}", existing.to_string_lossy()))
        }
        _ => OsString::from(new_path),
    };
    vec![(OsString::from("NODE_PATH"), value)]
}

fn parse_cmd_set(line: &str, var: &str) -> Option<String> {
    let trimmed = line.trim().trim_start_matches('@').trim();
    let prefix = format!("SET \"{}=", var.to_ascii_uppercase());
    let upper = trimmed.to_ascii_uppercase();
    if !upper.starts_with(&prefix) {
        return None;
    }
    let mut value = trimmed[prefix.len()..].to_string();
    if value.ends_with('"') {
        value.pop();
    }
    Some(value)
}

fn quoted_tokens(line: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current: Option<String> = None;
    for ch in line.chars() {
        if ch == '"' {
            if let Some(token) = current.take() {
                tokens.push(token);
            } else {
                current = Some(String::new());
            }
        } else if let Some(token) = current.as_mut() {
            token.push(ch);
        }
    }
    tokens
}

fn expand_cmd_path(raw: &str, dir: &Path) -> PathBuf {
    let dir = dir.to_string_lossy();
    let expanded = raw
        .replace("%dp0%", &dir)
        .replace("%DP0%", &dir)
        .replace("%~dp0", &dir)
        .replace("%~DP0", &dir);
    PathBuf::from(expanded)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn node_shim_invocation_bypasses_cmd() {
        let dir = temp_dir("node-shim");
        fs::create_dir_all(dir.join("node_modules/pkg/bin")).unwrap();
        let script = dir.join("node_modules/pkg/bin/cli.js");
        fs::write(&script, "").unwrap();
        let shim = dir.join("tool.cmd");
        fs::write(
            &shim,
            r#"@ECHO off
SETLOCAL
node "%dp0%\node_modules\pkg\bin\cli.js" %*
"#,
        )
        .unwrap();

        let invocation = cmd_shim_invocation(&shim, &[OsString::from("--version")]).unwrap();

        assert_eq!(invocation.program, PathBuf::from("node"));
        assert_eq!(
            fs::canonicalize(PathBuf::from(&invocation.args[0])).unwrap(),
            fs::canonicalize(script).unwrap()
        );
        assert_eq!(invocation.args[1], OsString::from("--version"));
        assert!(!invocation.uses_shell());
        fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn opencode_shim_preserves_no_arg_web_default() {
        let dir = temp_dir("opencode-shim");
        fs::create_dir_all(dir.join("node_modules/opencode-ai/bin")).unwrap();
        let exe = dir.join("node_modules/opencode-ai/bin/opencode.exe");
        fs::write(&exe, "").unwrap();
        let shim = dir.join("opencode.cmd");
        fs::write(
            &shim,
            r#"@ECHO off
SET "opencode_exe=%dp0%\node_modules\opencode-ai\bin\opencode.exe"
"#,
        )
        .unwrap();

        let invocation = cmd_shim_invocation(&shim, &[]).unwrap();

        assert_eq!(invocation.program, exe);
        assert_eq!(
            invocation.args,
            vec![
                OsString::from("web"),
                OsString::from("--hostname"),
                OsString::from("127.0.0.1"),
                OsString::from("--port"),
                OsString::from("4096"),
            ]
        );
        fs::remove_dir_all(dir).ok();
    }

    fn temp_dir(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("orchester-{name}-{}-{nanos}", std::process::id()))
    }
}
