//! Structured command intent parsing and classification.
//!
//! The classifier deliberately accepts an executable and an argument vector,
//! never a shell command string.  It keeps the original values for later audit
//! while using only a normalized executable basename for rule matching.

use std::collections::BTreeSet;
use std::ffi::{OsStr, OsString};
use std::fmt;
use std::path::PathBuf;

use thiserror::Error;

const MAX_PROGRAM_BYTES: usize = 4 * 1024;
const MAX_ARGUMENTS: usize = 256;
const MAX_ARGUMENT_BYTES: usize = 64 * 1024;

/// Coarse effects used by governance rules.  The policy layer refines these
/// into an allow/ask/deny decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CommandCategory {
    /// A command from the explicit read-only allowlist.
    ReadOnly,
    /// A command that can write project state without an external effect.
    WorkspaceWrite,
    /// A command that removes files or directories.
    Delete,
    /// A delete operation carrying a recursive flag.
    RecursiveDelete,
    /// A client that can contact a network endpoint.
    Network,
    /// A command that attempts to change effective privileges.
    PrivilegeEscalation,
    /// A Git operation that mutates repository history or metadata.
    GitWrite,
    /// A Git operation that can destroy or rewrite repository state.
    GitDestructive,
    /// A package manager operation that resolves or installs dependencies.
    PackageInstall,
    /// A shell or scripting interpreter.
    ShellInterpreter,
    /// An operation targeting system resources rather than the workspace.
    SystemDestructive,
    /// A wrapper that can hide the actual executable or alter its environment.
    UnsupportedWrapper,
    /// An argument vector containing shell composition/control syntax.
    Composite,
    /// The executable is not in a known safe or governed family.
    Unknown,
}

/// Parsed command intent.  `program` and `args` are preserved exactly; the
/// classifier never reconstructs them into a shell string.
#[derive(Clone, PartialEq, Eq)]
pub struct CommandIntent {
    pub program: PathBuf,
    pub args: Vec<OsString>,
    pub categories: BTreeSet<CommandCategory>,
}

impl fmt::Debug for CommandIntent {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let argument_bytes = self
            .args
            .iter()
            .map(|argument| argument.to_string_lossy().len())
            .sum::<usize>();
        formatter
            .debug_struct("CommandIntent")
            .field(
                "program_bytes",
                &self.program.as_os_str().to_string_lossy().len(),
            )
            .field("args_count", &self.args.len())
            .field("args_bytes", &argument_bytes)
            .field("categories", &self.categories)
            .finish()
    }
}

impl CommandIntent {
    /// Return the UTF-8 basename used for deterministic matching.
    pub fn executable_basename(&self) -> Option<&str> {
        self.program
            .as_os_str()
            .to_str()
            .and_then(executable_basename)
    }
}

/// Errors raised before a command can be classified.  Variants intentionally
/// contain no user-controlled text so they are safe to display in an audit
/// or approval surface.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum CommandParseError {
    #[error("command program is empty")]
    EmptyProgram,
    #[error("command program is not valid Unicode")]
    InvalidProgram,
    #[error("command program contains a NUL byte")]
    ProgramContainsNul,
    #[error("command program is too long")]
    ProgramTooLong,
    #[error("command has too many arguments")]
    TooManyArguments,
    #[error("command argument {index} is not valid Unicode")]
    InvalidArgument { index: usize },
    #[error("command argument {index} contains a NUL byte")]
    ArgumentContainsNul { index: usize },
    #[error("command argument {index} is too long")]
    ArgumentTooLong { index: usize },
    #[error("command executable basename is malformed")]
    MalformedBasename,
}

/// Parse and classify an executable plus its independent argument vector.
/// Invalid input is returned as an error; callers that need a governance
/// result should pass it through [`crate::harness::governance::PolicyEngine`],
/// which converts every parse error to DENY.
pub fn classify_command<P>(
    program: P,
    args: &[OsString],
) -> Result<CommandIntent, CommandParseError>
where
    P: AsRef<OsStr>,
{
    let program = program.as_ref();
    let program_text = program.to_str().ok_or(CommandParseError::InvalidProgram)?;
    if program_text.is_empty() {
        return Err(CommandParseError::EmptyProgram);
    }
    if program_text.contains('\0') {
        return Err(CommandParseError::ProgramContainsNul);
    }
    if program_text.len() > MAX_PROGRAM_BYTES {
        return Err(CommandParseError::ProgramTooLong);
    }
    let basename = executable_basename(program_text).ok_or(CommandParseError::MalformedBasename)?;
    if basename.is_empty() || basename == "." || basename == ".." {
        return Err(CommandParseError::MalformedBasename);
    }
    if args.len() > MAX_ARGUMENTS {
        return Err(CommandParseError::TooManyArguments);
    }

    let mut owned_args = Vec::with_capacity(args.len());
    let mut arg_text = Vec::with_capacity(args.len());
    for (index, arg) in args.iter().enumerate() {
        let arg = arg.as_os_str();
        let text = arg
            .to_str()
            .ok_or(CommandParseError::InvalidArgument { index })?;
        if text.contains('\0') {
            return Err(CommandParseError::ArgumentContainsNul { index });
        }
        if text.len() > MAX_ARGUMENT_BYTES {
            return Err(CommandParseError::ArgumentTooLong { index });
        }
        owned_args.push(arg.to_os_string());
        arg_text.push(text);
    }

    let categories = classify_basename(basename, &arg_text);
    Ok(CommandIntent {
        program: PathBuf::from(program),
        args: owned_args,
        categories,
    })
}

fn executable_basename(program: &str) -> Option<&str> {
    let basename = program.rsplit(['/', '\\']).next()?;
    (!basename.is_empty()).then_some(basename)
}

fn classify_basename(program: &str, args: &[&str]) -> BTreeSet<CommandCategory> {
    let mut categories = BTreeSet::new();
    let mut basename = program.to_ascii_lowercase();
    for suffix in [".exe", ".cmd", ".bat", ".com"] {
        if let Some(stripped) = basename.strip_suffix(suffix) {
            basename = stripped.to_owned();
            break;
        }
    }

    if is_shell_interpreter(&basename) {
        categories.insert(CommandCategory::ShellInterpreter);
        return categories;
    }
    if is_privilege_tool(&basename) {
        categories.insert(CommandCategory::PrivilegeEscalation);
        return categories;
    }
    if is_unsupported_wrapper(&basename) {
        categories.insert(CommandCategory::UnsupportedWrapper);
        return categories;
    }
    if has_external_hook(&basename, args) {
        categories.insert(CommandCategory::UnsupportedWrapper);
        return categories;
    }
    if contains_control_token(args) {
        categories.insert(CommandCategory::Composite);
        return categories;
    }
    if is_system_destructive(&basename, args) {
        categories.insert(CommandCategory::SystemDestructive);
        return categories;
    }
    if is_git(&basename) {
        classify_git(args, &mut categories);
        if !categories.is_empty() {
            return categories;
        }
    }
    if is_package_install(&basename, args) {
        categories.insert(CommandCategory::PackageInstall);
        return categories;
    }
    if is_network_client(&basename, args) {
        categories.insert(CommandCategory::Network);
        return categories;
    }
    if is_delete_command(&basename) {
        categories.insert(CommandCategory::Delete);
        if has_recursive_flag(args) {
            categories.insert(CommandCategory::RecursiveDelete);
            if has_root_target(args) {
                categories.insert(CommandCategory::SystemDestructive);
            }
        }
        return categories;
    }
    if is_workspace_writer(&basename, args) {
        categories.insert(CommandCategory::WorkspaceWrite);
        return categories;
    }
    if is_read_only_command(&basename, args) {
        categories.insert(CommandCategory::ReadOnly);
        return categories;
    }

    categories.insert(CommandCategory::Unknown);
    categories
}

fn is_shell_interpreter(program: &str) -> bool {
    matches!(
        program,
        "sh" | "bash"
            | "dash"
            | "zsh"
            | "fish"
            | "ksh"
            | "csh"
            | "tcsh"
            | "cmd"
            | "powershell"
            | "pwsh"
            | "wsl"
            | "python"
            | "python3"
            | "perl"
            | "ruby"
            | "node"
            | "deno"
            | "php"
            | "lua"
    )
}

fn is_privilege_tool(program: &str) -> bool {
    matches!(
        program,
        "sudo" | "sudoedit" | "doas" | "pkexec" | "su" | "runas" | "gsudo" | "elevate"
    )
}

fn is_unsupported_wrapper(program: &str) -> bool {
    matches!(program, "env" | "xargs" | "exec" | "command" | "busybox")
}

fn has_external_hook(program: &str, args: &[&str]) -> bool {
    args.iter().any(|arg| {
        let lower = arg.to_ascii_lowercase();
        match program {
            "rg" => {
                lower == "--pre"
                    || lower == "--pre-glob"
                    || lower.starts_with("--pre=")
                    || lower.starts_with("--pre-glob=")
            }
            "git" => {
                lower == "--ext-diff"
                    || lower == "--textconv"
                    || lower.starts_with("--ext-diff=")
                    || lower.starts_with("--textconv=")
            }
            _ => false,
        }
    })
}

fn is_system_destructive(program: &str, args: &[&str]) -> bool {
    if matches!(
        program,
        "shutdown"
            | "reboot"
            | "halt"
            | "poweroff"
            | "systemctl"
            | "service"
            | "sc"
            | "diskpart"
            | "mount"
            | "umount"
            | "mkfs"
            | "format"
            | "bcdedit"
            | "devcon"
            | "reg"
            | "registry"
    ) {
        return true;
    }
    // `find ... -delete` and shell-like delete switches are destructive even
    // though the executable itself is otherwise a read-only search tool.
    (program == "find" && args.iter().any(|arg| arg.eq_ignore_ascii_case("-delete")))
        || ((program == "del" || program == "erase")
            && has_recursive_flag(args)
            && has_root_target(args))
}

fn is_git(program: &str) -> bool {
    program == "git"
}

fn classify_git(args: &[&str], categories: &mut BTreeSet<CommandCategory>) {
    let Some(subcommand) = args
        .iter()
        .find(|arg| !arg.starts_with('-'))
        .map(|arg| arg.to_ascii_lowercase())
    else {
        categories.insert(CommandCategory::Unknown);
        return;
    };
    let destructive = match subcommand.as_str() {
        "reset" => args.iter().any(|arg| is_flag(arg, "--hard")),
        "clean" => args.iter().any(|arg| {
            let lower = arg.to_ascii_lowercase();
            lower.contains('f') || lower == "-x" || lower == "--force"
        }),
        "push" => args.iter().any(|arg| {
            let lower = arg.to_ascii_lowercase();
            lower == "--force" || lower == "-f" || lower.starts_with("--force=")
        }),
        "branch" => args
            .iter()
            .any(|arg| *arg == "-D" || *arg == "--delete --force"),
        "checkout" | "restore" => args.iter().any(|arg| *arg == "." || *arg == "--"),
        "rm" => args
            .iter()
            .any(|arg| arg.starts_with('-') && arg.contains('r')),
        "filter-branch" | "filter-repo" | "reflog" => true,
        _ => false,
    };
    if destructive {
        categories.insert(CommandCategory::GitDestructive);
    } else if matches!(
        subcommand.as_str(),
        "add"
            | "commit"
            | "mv"
            | "rm"
            | "reset"
            | "checkout"
            | "restore"
            | "branch"
            | "merge"
            | "rebase"
            | "tag"
            | "push"
            | "pull"
            | "fetch"
            | "clone"
            | "stash"
            | "config"
    ) {
        categories.insert(CommandCategory::GitWrite);
        if matches!(subcommand.as_str(), "push" | "pull" | "fetch" | "clone") {
            categories.insert(CommandCategory::Network);
        }
    } else if is_git_read_subcommand(&subcommand) {
        categories.insert(CommandCategory::ReadOnly);
    } else {
        categories.insert(CommandCategory::Unknown);
    }
}

fn is_git_read_subcommand(subcommand: &str) -> bool {
    matches!(
        subcommand,
        "status" | "log" | "diff" | "show" | "branch" | "tag" | "rev-parse" | "ls-files"
    )
}

fn is_package_install(program: &str, args: &[&str]) -> bool {
    let first = args
        .iter()
        .find(|arg| !arg.starts_with('-'))
        .map(|arg| arg.to_ascii_lowercase());
    let Some(first) = first else { return false };
    match program {
        "cargo" => matches!(first.as_str(), "add" | "install" | "update"),
        "npm" | "pnpm" => matches!(first.as_str(), "install" | "i" | "add" | "update"),
        "yarn" => matches!(first.as_str(), "install" | "add" | "upgrade"),
        "pip" | "pip3" | "uv" => matches!(first.as_str(), "install" | "add" | "sync"),
        "poetry" => matches!(first.as_str(), "install" | "add" | "update"),
        "gem" => first == "install",
        "go" => matches!(first.as_str(), "get" | "install"),
        "dotnet" => first == "add" && args.iter().any(|arg| arg.eq_ignore_ascii_case("package")),
        "apt" | "apt-get" | "dnf" | "yum" | "pacman" | "brew" | "choco" | "winget" => {
            matches!(first.as_str(), "install" | "add" | "upgrade" | "update")
        }
        _ => false,
    }
}

fn is_network_client(program: &str, args: &[&str]) -> bool {
    if matches!(
        program,
        "curl"
            | "wget"
            | "aria2c"
            | "scp"
            | "sftp"
            | "ssh"
            | "nc"
            | "ncat"
            | "telnet"
            | "ftp"
            | "http"
            | "aws"
            | "az"
            | "gcloud"
    ) {
        return true;
    }
    (program == "docker" || program == "podman")
        && args.iter().any(|arg| {
            matches!(
                arg.to_ascii_lowercase().as_str(),
                "pull" | "push" | "login" | "run" | "build"
            )
        })
}

fn is_delete_command(program: &str) -> bool {
    matches!(program, "rm" | "rmdir" | "rd" | "del" | "erase" | "unlink")
}

fn is_workspace_writer(program: &str, args: &[&str]) -> bool {
    match program {
        "mkdir" | "md" | "touch" | "cp" | "copy" | "mv" | "move" | "ln" => true,
        "cargo" => args.iter().any(|arg| {
            matches!(
                arg.to_ascii_lowercase().as_str(),
                "build" | "check" | "test" | "run" | "fmt"
            )
        }),
        "npm" | "pnpm" | "yarn" => args.iter().any(|arg| {
            matches!(
                arg.to_ascii_lowercase().as_str(),
                "run" | "test" | "exec" | "pack"
            )
        }),
        _ => false,
    }
}

fn is_read_only_command(program: &str, args: &[&str]) -> bool {
    match program {
        "ls" | "dir" | "pwd" | "cat" | "type" | "head" | "tail" | "rg" | "grep" | "which"
        | "where" | "stat" | "file" | "wc" | "echo" | "printf" | "true" | "false" | "whoami"
        | "id" | "uname" => true,
        "find" => !args.iter().any(|arg| {
            let lower = arg.to_ascii_lowercase();
            lower == "-delete" || lower == "-exec" || lower == "-execdir"
        }),
        _ => false,
    }
}

fn contains_control_token(args: &[&str]) -> bool {
    args.iter().any(|arg| {
        arg.chars()
            .any(|character| matches!(character, ';' | '|' | '>' | '<' | '&' | '`' | '\n' | '\r'))
            || arg.contains("$(")
    })
}

fn has_recursive_flag(args: &[&str]) -> bool {
    args.iter().any(|arg| {
        let lower = arg.to_ascii_lowercase();
        lower == "--recursive"
            || lower == "/s"
            || lower == "-r"
            || lower == "-R"
            || (lower.starts_with('-') && !lower.starts_with("--") && lower.contains('r'))
    })
}

fn has_root_target(args: &[&str]) -> bool {
    args.iter()
        .filter(|arg| !arg.starts_with('-'))
        .any(|arg| is_root_target(arg))
}

fn is_root_target(value: &str) -> bool {
    let trimmed = value.trim_matches(['\'', '"']);
    if matches!(trimmed, "." | ".." | "./" | "../" | "*") {
        return true;
    }
    let normalized = trimmed.replace('\\', "/");
    if normalized.chars().all(|character| character == '/') {
        return true;
    }
    if normalized.starts_with("/*")
        || normalized.starts_with("../")
        || normalized.ends_with(":/")
        || normalized.ends_with(":/*")
    {
        return true;
    }
    let bytes = normalized.as_bytes();
    bytes.len() == 3 && bytes[1] == b':' && bytes[2] == b'/'
}

fn is_flag(value: &str, expected: &str) -> bool {
    value.eq_ignore_ascii_case(expected)
}
