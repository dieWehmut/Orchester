use std::fs;
use std::path::PathBuf;
use std::time::Duration;

use orchester_protokoll::{
    ActionId, AgentAction, EventId, HarnessEvent, HarnessEventKind, RunId, StepId,
};

use super::*;
use crate::harness::barrier::StartedTool;
use crate::harness::process_tree::ProcessTree;

fn root(name: &str) -> PathBuf {
    let root =
        std::env::temp_dir().join(format!("orchester-process-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(root.join("work")).unwrap();
    root
}

fn started(action: AgentAction) -> StartedTool {
    let action_id = ActionId::from("process-action");
    StartedTool::new(
        HarnessEvent::new_for_test(
            EventId::from("process-event"),
            RunId::from("process-run"),
            StepId::from("process-step"),
            1,
            HarnessEventKind::ToolStarted { action_id },
        ),
        action,
    )
}

fn command(program: &str, args: &[&str], cwd: &str) -> AgentAction {
    AgentAction::RunCommand {
        program: program.into(),
        args: args.iter().map(|argument| (*argument).into()).collect(),
        cwd: Some(cwd.into()),
    }
}

#[cfg(windows)]
fn slow_process() -> AgentAction {
    command("ping", &["-n", "10", "-w", "1000", "127.0.0.1"], "work")
}

#[cfg(unix)]
fn slow_process() -> AgentAction {
    command("sleep", &["5"], "work")
}

#[tokio::test]
async fn process_tree_attaches_and_terminates_a_running_child() {
    let tree = ProcessTree::new().expect("create process tree");
    let mut command = if cfg!(windows) {
        let mut command = tokio::process::Command::new("ping");
        command.args(["-n", "10", "-w", "1000", "127.0.0.1"]);
        command
    } else {
        let mut command = tokio::process::Command::new("sleep");
        command.arg("5");
        command
    };
    tree.configure_command(&mut command);
    let mut child = command.spawn().expect("spawn child");
    tree.attach(&child).expect("attach child");

    tree.terminate(&mut child);
    let status = tokio::time::timeout(Duration::from_secs(2), child.wait())
        .await
        .expect("termination deadline")
        .expect("wait for child");

    assert!(!status.success());
}

#[tokio::test]
async fn runs_a_permit_bound_command_and_bounds_both_streams() {
    let root = root("safe");
    let runner = GovernedProcessRunner::new(
        &root,
        ProcessLimits {
            max_output_bytes: 1,
            ..ProcessLimits::default()
        },
    )
    .unwrap();
    let result = runner
        .execute(
            started(command("whoami", &[], "work")),
            CancellationToken::new(),
        )
        .await
        .unwrap();
    assert!(matches!(result.termination, CommandTermination::Exited(0)));
    assert!(result.stdout.truncated() || result.stdout.total_bytes() <= 1);
    drop(runner);
    fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn cancellation_before_spawn_is_fail_closed() {
    let root = root("cancel");
    let runner = GovernedProcessRunner::new(&root, ProcessLimits::default()).unwrap();
    let cancellation = CancellationToken::new();
    cancellation.cancel();
    let error = runner
        .execute(started(command("whoami", &[], "work")), cancellation)
        .await
        .unwrap_err();
    assert_eq!(error, ProcessError::CancelledBeforeStart);
    drop(runner);
    fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn shell_secret_environment_and_wrong_actions_never_spawn() {
    let root = root("reject");
    let runner = GovernedProcessRunner::new(&root, ProcessLimits::default()).unwrap();
    let shell = if cfg!(windows) { "cmd" } else { "sh" };
    assert!(matches!(
        runner
            .execute(
                started(command(shell, &[], "work")),
                CancellationToken::new()
            )
            .await,
        Err(ProcessError::PolicyDenied)
    ));
    let secret_env = ProcessSpec::new("whoami", "work").env("OPENAI_API_KEY", "sk-test");
    assert_eq!(
        validate_spec(&secret_env),
        Err(ProcessError::SecretEnvironment)
    );
    assert!(matches!(
        runner
            .execute(
                started(AgentAction::ReadFile {
                    path: "src/lib.rs".into(),
                    start_line: None,
                    end_line: None,
                }),
                CancellationToken::new(),
            )
            .await,
        Err(ProcessError::WrongAction)
    ));
    drop(runner);
    fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn cwd_must_be_inside_the_capability_workspace() {
    let root = root("cwd");
    let runner = GovernedProcessRunner::new(&root, ProcessLimits::default()).unwrap();
    let error = runner
        .execute(
            started(command("whoami", &[], "../")),
            CancellationToken::new(),
        )
        .await
        .unwrap_err();
    assert_eq!(error, ProcessError::InvalidCwd);
    drop(runner);
    fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn timeout_terminates_a_running_process() {
    let root = root("timeout");
    let runner = GovernedProcessRunner::new(
        &root,
        ProcessLimits {
            timeout: Duration::from_millis(50),
            poll_interval: Duration::from_millis(5),
            ..ProcessLimits::default()
        },
    )
    .unwrap();
    let result = runner
        .execute(started(slow_process()), CancellationToken::new())
        .await
        .unwrap();
    assert_eq!(result.termination, CommandTermination::TimedOut);
    assert!(result.elapsed < Duration::from_secs(2));
    drop(runner);
    fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn cancellation_terminates_a_process_after_spawn() {
    let root = root("running-cancel");
    let runner = GovernedProcessRunner::new(&root, ProcessLimits::default()).unwrap();
    let cancellation = CancellationToken::new();
    let trigger = cancellation.clone();
    let cancel = async move {
        tokio::time::sleep(Duration::from_millis(50)).await;
        trigger.cancel();
    };
    let (result, ()) = tokio::join!(
        runner.execute(started(slow_process()), cancellation),
        cancel
    );
    assert_eq!(result.unwrap().termination, CommandTermination::Cancelled);
    drop(runner);
    fs::remove_dir_all(root).unwrap();
}
