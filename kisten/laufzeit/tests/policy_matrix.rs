use std::ffi::{OsStr, OsString};

use orchester_laufzeit::harness::governance::{
    classify_command, CommandCategory, EffectClass, PolicyEngine, Risk,
};
use orchester_protokoll::{AgentAction, PolicyDecision};

fn engine() -> PolicyEngine {
    PolicyEngine::new()
}

fn command(program: &str, args: &[&str]) -> AgentAction {
    AgentAction::RunCommand {
        program: program.to_owned(),
        args: args.iter().map(|arg| (*arg).to_owned()).collect(),
        cwd: None,
    }
}

#[test]
fn deterministic_policy_matrix() {
    let cases = [
        (
            AgentAction::ReadFile {
                path: "src/lib.rs".into(),
                start_line: None,
                end_line: None,
            },
            PolicyDecision::Allow,
            "workspace.read",
            EffectClass::ReadOnlyIdempotent,
        ),
        (
            command("git", &["commit", "-m", "x"]),
            PolicyDecision::Ask,
            "git.write",
            EffectClass::WorkspaceMutation,
        ),
        (
            command("cargo", &["add", "serde"]),
            PolicyDecision::Ask,
            "dependency.install",
            EffectClass::ExternalEffect,
        ),
        (
            command("powershell", &["-Command", "Get-ChildItem"]),
            PolicyDecision::Deny,
            "shell.interpreter",
            EffectClass::ExternalEffect,
        ),
        (
            command("rm", &["-rf", "/"]),
            PolicyDecision::Deny,
            "system.destructive",
            EffectClass::ExternalEffect,
        ),
    ];

    for (action, decision, rule_id, effect) in cases {
        let result = engine().evaluate(&action).expect("known action evaluates");
        assert_eq!(result.decision, decision, "decision for {rule_id}");
        assert_eq!(result.rule_id, rule_id, "rule for {rule_id}");
        assert_eq!(result.effect, effect, "effect for {rule_id}");
    }
}

#[test]
fn safe_command_is_allow_and_preserves_original_arguments() {
    let program = OsStr::new(r"C:\Tools\RG.EXE");
    let args = [OsString::from("--glob"), OsString::from("*.rs")];
    let intent = classify_command(program, &args).expect("valid command");
    assert_eq!(intent.program, std::path::PathBuf::from(program));
    assert_eq!(intent.args, args);
    assert!(intent.categories.contains(&CommandCategory::ReadOnly));

    let result = engine().evaluate_command(program, &args);
    assert_eq!(result.decision, PolicyDecision::Allow);
    assert_eq!(result.rule_id, "workspace.read");
    assert_eq!(result.risk, Risk::Low);
}

#[test]
fn command_categories_cover_network_delete_privilege_git_and_package() {
    let network = classify_command(
        OsStr::new("curl"),
        &[OsString::from("https://example.test")],
    )
    .expect("network command parses");
    assert!(network.categories.contains(&CommandCategory::Network));

    let delete = classify_command(OsStr::new("rm"), &[OsString::from("notes.txt")])
        .expect("delete command parses");
    assert!(delete.categories.contains(&CommandCategory::Delete));

    let privilege = classify_command(OsStr::new("sudo"), &[OsString::from("id")])
        .expect("privilege command parses");
    assert!(privilege
        .categories
        .contains(&CommandCategory::PrivilegeEscalation));

    let git = classify_command(
        OsStr::new("git"),
        &[OsString::from("reset"), OsString::from("--hard")],
    )
    .expect("git command parses");
    assert!(git.categories.contains(&CommandCategory::GitDestructive));

    let package = classify_command(
        OsStr::new("cargo"),
        &[OsString::from("add"), OsString::from("serde")],
    )
    .expect("package command parses");
    assert!(package
        .categories
        .contains(&CommandCategory::PackageInstall));
}

#[test]
fn ordinary_recursive_delete_is_ask_but_root_delete_is_denied() {
    let workspace = engine()
        .evaluate(&command("rm", &["-rf", "build"]))
        .unwrap();
    assert_eq!(workspace.decision, PolicyDecision::Ask);
    assert_eq!(workspace.rule_id, "filesystem.delete");

    let root = engine().evaluate(&command("rm", &["-rf", "/"])).unwrap();
    assert_eq!(root.decision, PolicyDecision::Deny);
    assert_eq!(root.rule_id, "system.destructive");
    assert_eq!(root.risk, Risk::Critical);
}

#[test]
fn platform_root_aliases_and_wildcards_are_denied() {
    for action in [
        command("rm", &["-rf", "//"]),
        command("rmdir", &["/s", r"C:\"]),
        command("del", &["/s", r"C:\*"]),
        command("rm", &["-rf", "C:/*"]),
    ] {
        let result = engine().evaluate(&action).unwrap();
        assert_eq!(result.decision, PolicyDecision::Deny);
        assert_eq!(result.rule_id, "system.destructive");
    }
}

#[test]
fn shell_wrappers_and_control_tokens_fail_closed() {
    let shell = engine()
        .evaluate(&command(r"C:\Windows\System32\cmd.exe", &["/c", "dir"]))
        .unwrap();
    assert_eq!(shell.decision, PolicyDecision::Deny);
    assert_eq!(shell.rule_id, "shell.interpreter");

    let wrapper = engine()
        .evaluate(&command("env", &["TOKEN=value", "git", "status"]))
        .unwrap();
    assert_eq!(wrapper.decision, PolicyDecision::Deny);
    assert_eq!(wrapper.rule_id, "command.wrapper");

    let composite = engine()
        .evaluate(&command("git", &["status", "&&", "rm", "-rf", "/"]))
        .unwrap();
    assert_eq!(composite.decision, PolicyDecision::Deny);
    assert_eq!(composite.rule_id, "command.composite");

    let attached = engine()
        .evaluate(&command("git", &["status;rm", "-rf", "/"]))
        .unwrap();
    assert_eq!(attached.decision, PolicyDecision::Deny);
    assert_eq!(attached.rule_id, "command.composite");
}

#[test]
fn unknown_commands_and_parse_failures_never_allow() {
    let unknown = engine().evaluate(&command("not-a-real-tool", &[])).unwrap();
    assert_eq!(unknown.decision, PolicyDecision::Deny);
    assert_eq!(unknown.rule_id, "command.unknown");

    let empty = engine().evaluate_command(OsStr::new(""), &[]);
    assert_eq!(empty.decision, PolicyDecision::Deny);
    assert_eq!(empty.rule_id, "command.parse");

    let nul = engine().evaluate_command(OsStr::new("git\0status"), &[]);
    assert_eq!(nul.decision, PolicyDecision::Deny);
    assert_eq!(nul.rule_id, "command.parse");

    let too_many = vec![OsString::from("arg"); 257];
    let result = engine().evaluate_command(OsStr::new("git"), &too_many);
    assert_eq!(result.decision, PolicyDecision::Deny);
    assert_eq!(result.rule_id, "command.parse");

    let too_long = vec![OsString::from("x".repeat(65_537))];
    let result = engine().evaluate_command(OsStr::new("git"), &too_long);
    assert_eq!(result.decision, PolicyDecision::Deny);
    assert_eq!(result.rule_id, "command.parse");

    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStringExt;
        let invalid = OsString::from_vec(vec![b'g', b'i', 0x80]);
        let result = engine().evaluate_command(&invalid, &[]);
        assert_eq!(result.decision, PolicyDecision::Deny);
        assert_eq!(result.rule_id, "command.parse");

        let invalid_arg = OsString::from_vec(vec![0x80]);
        let result = engine().evaluate_command(OsStr::new("git"), &[invalid_arg]);
        assert_eq!(result.decision, PolicyDecision::Deny);
        assert_eq!(result.rule_id, "command.parse");
    }

    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStringExt;
        let invalid = OsString::from_wide(&[0xD800]);
        let result = engine().evaluate_command(&invalid, &[]);
        assert_eq!(result.decision, PolicyDecision::Deny);
        assert_eq!(result.rule_id, "command.parse");
    }
}

#[test]
fn network_and_dependency_effects_require_approval() {
    let network = engine()
        .evaluate(&command("curl", &["https://example.test"]))
        .unwrap();
    assert_eq!(network.decision, PolicyDecision::Ask);
    assert_eq!(network.rule_id, "network.external");
    assert_eq!(network.effect, EffectClass::ExternalEffect);

    let package = engine()
        .evaluate(&command("npm", &["install", "left-pad"]))
        .unwrap();
    assert_eq!(package.decision, PolicyDecision::Ask);
    assert_eq!(package.rule_id, "dependency.install");
    assert_eq!(package.effect, EffectClass::ExternalEffect);
}

#[test]
fn policy_explanations_do_not_echo_model_controlled_arguments() {
    let marker = "authorization-secret-marker";
    let result = engine()
        .evaluate(&command("curl", &[marker]))
        .expect("known action evaluates");
    assert!(!result.reason.contains(marker));
    assert!(!result.rule_id.contains(marker));
}

#[test]
fn explicit_approval_action_always_pauses_for_the_owner() {
    let result = engine()
        .evaluate(&AgentAction::RequestApproval {
            reason: "confirm the next step".into(),
        })
        .unwrap();
    assert_eq!(result.decision, PolicyDecision::Ask);
    assert_eq!(result.rule_id, "approval.explicit_checkpoint");
    assert_eq!(result.effect, EffectClass::ReadOnlyIdempotent);
}

#[test]
fn command_debug_does_not_echo_program_or_arguments() {
    let secret_program = OsStr::new(r"C:\private\api-key-tool.exe");
    let secret_arg = OsString::from("authorization-secret-marker");
    let intent = classify_command(secret_program, &[secret_arg]).expect("command parses");
    let debug = format!("{intent:?}");
    assert!(!debug.contains("api-key-tool"));
    assert!(!debug.contains("authorization-secret-marker"));
}

#[test]
fn executable_argument_hooks_fail_closed() {
    let rg_pre = engine()
        .evaluate(&command("rg", &["--pre", "cat", "needle"]))
        .unwrap();
    assert_eq!(rg_pre.decision, PolicyDecision::Deny);
    assert_eq!(rg_pre.rule_id, "command.wrapper");

    let rg_pre_glob = engine()
        .evaluate(&command("rg", &["--pre-glob", "*.gz", "needle"]))
        .unwrap();
    assert_eq!(rg_pre_glob.decision, PolicyDecision::Deny);
    assert_eq!(rg_pre_glob.rule_id, "command.wrapper");

    let git_ext_diff = engine()
        .evaluate(&command("git", &["diff", "--ext-diff"]))
        .unwrap();
    assert_eq!(git_ext_diff.decision, PolicyDecision::Deny);
    assert_eq!(git_ext_diff.rule_id, "command.wrapper");

    let git_textconv = engine()
        .evaluate(&command("git", &["log", "--textconv"]))
        .unwrap();
    assert_eq!(git_textconv.decision, PolicyDecision::Deny);
    assert_eq!(git_textconv.rule_id, "command.wrapper");
}
