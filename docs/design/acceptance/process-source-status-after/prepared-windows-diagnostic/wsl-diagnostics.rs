use std::{fs::{self, File}, process::{Command, Stdio}, time::{Duration, Instant, SystemTime, UNIX_EPOCH}};
use std::os::windows::process::CommandExt;

struct OwnedChild(std::process::Child);
impl Drop for OwnedChild { fn drop(&mut self) { let _ = self.0.kill(); let _ = self.0.wait(); } }

struct Query { status: &'static str, elapsed: u128, exit: Option<i32>, data: Vec<u8>, codes: Vec<String> }
fn decode(raw: &[u8]) -> String {
    if raw.contains(&0) { String::from_utf16_lossy(&raw.chunks_exact(2).map(|b| u16::from_le_bytes([b[0], b[1]])).collect::<Vec<_>>()) }
    else { String::from_utf8_lossy(raw).into_owned() }
}
fn read_query(mode: &str, operation: &str, args: &[&str], no_window: bool, inherit_input: bool) -> Query {
    let output = format!("diagnostic-{mode}-{operation}.stdout");
    let error = format!("diagnostic-{mode}-{operation}.stderr");
    let start = Instant::now();
    let mut command = Command::new("wsl.exe");
    command.args(args).creation_flags(if no_window {0x08000000} else {0});
    command.stdin(if inherit_input {Stdio::inherit()} else {Stdio::null()});
    let result = (|| -> std::io::Result<Query> {
        command.stdout(File::create(&output)?).stderr(File::create(&error)?);
        let mut child = OwnedChild(command.spawn()?);
        loop {
            match child.0.try_wait()? {
                Some(status) => {
                    let data = fs::read(&output)?;
                    let raw_error = fs::read(&error)?;
                    let safe_codes = decode(&data) + &decode(&raw_error);
                    let codes = safe_codes.split(|c: char| !(c.is_ascii_alphanumeric() || matches!(c, '/' | '_')))
                        .filter(|s| s.starts_with("Wsl/") || *s == "E_INVALIDARG" || (s.starts_with("0x") && s.len() == 10 && s[2..].chars().all(|c| c.is_ascii_hexdigit())))
                        .take(8).map(str::to_owned).collect();
                    return Ok(Query {status: if start.elapsed() >= Duration::from_secs(3) {"timeout"} else if status.success() {"available"} else {"command_failed"}, elapsed: start.elapsed().as_millis(), exit: status.code(), data, codes});
                }
                None if start.elapsed() < Duration::from_secs(3) && fs::metadata(&output)?.len() <= 1024 * 1024 => {}
                None => {
                    let _ = child.0.kill(); let _ = child.0.wait();
                    return Ok(Query {status: "timeout", elapsed: start.elapsed().as_millis(), exit: None, data: Vec::new(), codes: Vec::new()});
                }
            }
            std::thread::sleep(Duration::from_millis(20));
        }
    })();
    let _ = fs::remove_file(&output); let _ = fs::remove_file(&error);
    result.unwrap_or(Query {status: "read_failed", elapsed: start.elapsed().as_millis(), exit: None, data: Vec::new(), codes: Vec::new()})
}
fn query_json(operation: &str, query: &Query) -> String {
    let codes = query.codes.iter().map(|code| format!("\"{code}\"")).collect::<Vec<_>>().join(",");
    let counts = if operation == "processes" && query.status == "available" {
        let text = decode(&query.data);
        let counts = ["codex", "claude", "opencode", "deepseek"].map(|name| text.lines().filter(|line| line.trim() == name).count());
        format!(r#"{{"codex":{},"claude":{},"opencode":{},"deepseek":{}}}"#, counts[0], counts[1], counts[2], counts[3])
    } else { "null".to_owned() };
    format!(r#"{{"operation":"{}","status":"{}","elapsed_ms":{},"exit_code":{},"error_codes":[{}],"provider_counts":{}}}"#, operation, query.status, query.elapsed, query.exit.map(|n| n.to_string()).unwrap_or("null".into()), codes, counts)
}
fn main() {
    let observed = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis();
    let mut modes = Vec::new();
    for (mode, no_window, inherit_input) in [("production_no_window_null_input",true,false),("inherited_console_null_input",false,false),("inherited_console_inherited_input",false,true)] {
        let list = read_query(mode, "enumeration", &["--list", "--running", "--quiet"], no_window, inherit_input);
        let mut queries = vec![query_json("enumeration", &list)];
        let names: Vec<String> = if list.status == "available" { decode(&list.data).lines().map(|s| s.trim().trim_start_matches('\u{feff}').to_owned()).filter(|s| !s.is_empty() && !s.contains(['/', '\\'])).collect() } else { Vec::new() };
        for (index, name) in names.iter().enumerate() {
            let operation = format!("processes_{index}");
            let query = read_query(mode, &operation, &["--distribution", name, "--exec", "sh", "-c", r#"for p in /proc/[0-9]*/comm; do IFS= read -r n < "$p" 2>/dev/null || continue; case "$n" in codex|claude|opencode|deepseek) printf '%s\n' "$n";; esac; done"#], no_window, inherit_input);
            queries.push(query_json("processes", &query));
        }
        modes.push(format!(r#"{{"mode":"{}","running_distribution_count":{},"queries":[{}]}}"#, mode, if list.status == "available" {names.len().to_string()} else {"null".into()}, queries.join(",")));
    }
    println!(r#"{{"observed_at_epoch_ms":{},"scope":"Only running WSL distribution enumeration and existing process names; no argv, prompt, credential, agent or model","role":"Independent handle and argument diagnostic; production source recorded separately","modes":[{}]}}"#, observed, modes.join(","));
}
