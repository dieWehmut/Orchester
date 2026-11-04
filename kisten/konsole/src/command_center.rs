//! Full-window, read-only task overview; only validated managed handles resume.
use crate::command_center_data::{self, Task, TaskStatus};
use crate::interactive::ChatSession;
use crossterm::event::{Event, KeyCode, KeyEventKind, KeyModifiers};
use std::{
    io,
    time::{Duration, Instant},
};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

pub enum Action {
    Back,
    Compose,
    Resume(String),
}
const FILTERS: [&str; 5] = ["All", "Needs you", "Working", "Ready", "Inactive"];
fn matches(task: &Task, filter: usize) -> bool {
    match filter {
        1 => matches!(task.status, TaskStatus::NeedsYou),
        2 => matches!(task.status, TaskStatus::Working),
        3 => matches!(task.status, TaskStatus::Ready),
        4 => matches!(task.status, TaskStatus::Inactive),
        _ => true,
    }
}
fn status(task: &Task) -> &'static str {
    match task.status {
        TaskStatus::NeedsYou => "Needs you",
        TaskStatus::Working => "Working",
        TaskStatus::Ready => "Ready",
        TaskStatus::Inactive => "Inactive",
    }
}
fn fit(text: &str, width: usize) -> String {
    let mut out = String::new();
    let mut used = 0;
    for c in text.chars().filter(|c| !c.is_control()) {
        let n = c.width().unwrap_or(0);
        if used + n > width {
            break;
        }
        out.push(c);
        used += n;
    }
    out
}
fn padded(text: &str, width: usize) -> String {
    let text = fit(text, width);
    let padding = width.saturating_sub(text.width());
    format!("{text}{}", " ".repeat(padding))
}
fn filtered(tasks: &[Task], filter: usize) -> Vec<&Task> {
    {
        let mut rows: Vec<_> = tasks.iter().filter(|t| matches(t, filter)).collect();
        rows.sort_by(|a, b| {
            a.project
                .cmp(&b.project)
                .then(a.project_key.cmp(&b.project_key))
        });
        rows
    }
}

pub fn run(chat: &mut ChatSession) -> io::Result<Action> {
    let mut tasks = Vec::new();
    let mut error = String::new();
    let mut filter = 0;
    let mut selected = 0;
    let mut detail = None;
    let mut scroll = 0;
    let mut refresh = Instant::now() - Duration::from_secs(6);
    let (sender, receiver) = std::sync::mpsc::channel();
    let mut loading = false;
    let mut dimensions = (0, 0);
    let mut dirty = true;
    loop {
        if refresh.elapsed() >= Duration::from_secs(5) && detail.is_none() && !loading {
            loading = true;
            refresh = Instant::now();
            let sender = sender.clone();
            std::thread::spawn(move || {
                let _ = sender.send(command_center_data::load());
            });
        }
        if let Ok(result) = receiver.try_recv() {
            loading = false;
            let selected_id = filtered(&tasks, filter).get(selected).map(|t| t.id.clone());
            match result {
                Ok(next) => {
                    tasks = next.tasks;
                    error = next.warnings.join(" ");
                }
                Err(e) => error = format!("Refresh failed: {e}"),
            }
            selected = selected_id
                .and_then(|id| filtered(&tasks, filter).iter().position(|t| t.id == id))
                .unwrap_or(0);
            refresh = Instant::now();
            dirty = true;
        }
        let size = chat.viewport();
        if size != dimensions {
            dimensions = size;
            dirty = true;
        }
        let rows = filtered(&tasks, filter);
        selected = selected.min(rows.len().saturating_sub(1));
        if let Some(text) = detail.as_deref() {
            scroll = scroll.min(detail_scroll_limit(text, dimensions));
        }
        if dirty {
            let frame = render(
                &tasks,
                filter,
                selected,
                dimensions,
                detail.as_deref(),
                scroll,
                &error,
            );
            let frame = if loading && tasks.is_empty() && error.is_empty() {
                let mut lines: Vec<_> = frame.lines().map(str::to_owned).collect();
                if lines.len() > 4 {
                    lines[4] = fit("Loading local tasks…", dimensions.0);
                }
                lines.join("\n")
            } else {
                frame
            };
            chat.present_raw(frame.as_bytes())?;
            dirty = false;
        }
        let Some(event) = chat.try_read_input()? else {
            std::thread::sleep(Duration::from_millis(30));
            continue;
        };
        let Event::Key(key) = event else {
            dirty = true;
            continue;
        };
        if key.kind == KeyEventKind::Release {
            continue;
        }
        dirty = true;
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            return Ok(Action::Back);
        }
        if detail.is_some() {
            match key.code {
                KeyCode::Esc => {
                    detail = None;
                    scroll = 0;
                }
                KeyCode::Down => scroll += 1,
                KeyCode::Up => scroll = scroll.saturating_sub(1),
                KeyCode::PageDown => scroll += dimensions.1.saturating_sub(5),
                KeyCode::PageUp => scroll = scroll.saturating_sub(dimensions.1.saturating_sub(5)),
                _ => {}
            }
            continue;
        }
        match key.code {
            KeyCode::Esc => return Ok(Action::Back),
            KeyCode::Char('n') => return Ok(Action::Compose),
            KeyCode::Tab => {
                filter = if key.modifiers.contains(KeyModifiers::SHIFT) { (filter + 4) % 5 } else { (filter + 1) % 5 };
                selected = 0;
            }
            KeyCode::BackTab => { filter = (filter + 4) % 5; selected = 0; }
            KeyCode::Down => selected = (selected + 1).min(rows.len().saturating_sub(1)),
            KeyCode::Up => selected = selected.saturating_sub(1),
            KeyCode::Home => selected = 0,
            KeyCode::End => selected = rows.len().saturating_sub(1),
            KeyCode::Char('r') => {
                if let Some(handle) = rows.get(selected).and_then(|t| t.resume_handle.clone()) { return Ok(Action::Resume(handle)); }
            }
            KeyCode::Enter => {
                if let Some(task) = rows.get(selected) { detail = Some(command_center_data::detail(task).unwrap_or_else(|e| e)); scroll = 0; }
            }
            KeyCode::Char('?') => detail = Some("Task command center\n\nTab / Shift+Tab   Filter by status\nUp / Down         Select a task\nEnter             Read transcript and details\nr                 Resume a resumable managed task\nn                 Compose a new prompt\nEsc               Back\n\nExternal sessions open read-only. Status comes from recorded activity, not process guesses.".into()),
            _ => {}
        }
    }
}

fn render(
    tasks: &[Task],
    filter: usize,
    selected: usize,
    (width, height): (usize, usize),
    detail: Option<&str>,
    scroll: usize,
    error: &str,
) -> String {
    if height == 0 || width == 0 {
        return String::new();
    }
    let mut lines = vec![if error.is_empty() {
        format!("\x1b[1m{}\x1b[0m", fit("Agent command center", width))
    } else {
        fit(&format!("Agent command center · {error}"), width)
    }];
    let filters = FILTERS
        .iter()
        .enumerate()
        .map(|(i, label)| {
            format!(
                "{}{} {}{}",
                if i == filter { "[" } else { "" },
                label,
                tasks.iter().filter(|t| matches(t, i)).count(),
                if i == filter { "]" } else { "" }
            )
        })
        .collect::<Vec<_>>()
        .join("  ");
    lines.push(if filters.width() <= width {
        filters
    } else {
        fit(
            &format!(
                "[{} {}] · Tab filter",
                FILTERS[filter],
                tasks.iter().filter(|t| matches(t, filter)).count()
            ),
            width,
        )
    });
    lines.push("─".repeat(width));
    let body = height.saturating_sub(4);
    if let Some(detail) = detail {
        let paragraphs = detail
            .lines()
            .flat_map(|line| wrap(line, width))
            .collect::<Vec<_>>();
        let start = scroll.min(paragraphs.len().saturating_sub(body));
        lines.extend(paragraphs.into_iter().skip(start).take(body));
    } else {
        let rows = filtered(tasks, filter);
        let chosen = rows.get(selected).copied();
        let mut project_counts = std::collections::HashMap::new();
        for task in &rows {
            *project_counts.entry(task.project_key.as_str()).or_insert(0usize) += 1;
        }
        let right = if width >= 110 { width / 3 } else { 0 };
        let left = width.saturating_sub(right + usize::from(right > 0));
        let mut list = vec![(
            false,
            if left >= 62 {
                format!(
                    "{} {} {}",
                    padded("Tasks", left - 29),
                    padded("Status", 12),
                    "Updated"
                )
            } else {
                "Tasks".into()
            },
        )];
        let mut selected_line = 0;
        let mut project_key = "";
        for (i, task) in rows.iter().enumerate() {
            if task.project_key != project_key {
                project_key = &task.project_key;
                list.push((
                    false,
                    format!(
                        "{}  {}",
                        task.project,
                        project_counts[project_key]
                    ),
                ));
            }
            if i == selected {
                selected_line = list.len();
            }
            let label = format!("{} {}", if i == selected { ">" } else { " " }, task.title);
            let row = if left >= 62 {
                format!(
                    "{} {} {}",
                    padded(&label, left - 29),
                    padded(status(task), 12),
                    fit(&task.updated, 15)
                )
            } else {
                format!("{label} · {}", status(task))
            };
            list.push((i == selected, row));
        }
        if rows.is_empty() {
            list.push((
                false,
                if error.is_empty() {
                    "No tasks in this filter.".into()
                } else {
                    format!("Could not load tasks: {error}")
                },
            ));
        }
        let start = selected_line.saturating_sub(body.saturating_sub(1));
        let details = chosen
            .map(|t| {
                vec![
                    "Task details".into(),
                    t.title.clone(),
                    status(t).into(),
                    String::new(),
                    format!("Model: {}", t.model),
                    format!("Reasoning: {}", t.reasoning),
                    String::new(),
                    "Project".into(),
                    t.project.clone(),
                    String::new(),
                    "Prompt".into(),
                    t.prompt.clone(),
                    String::new(),
                    if t.resume_handle.is_some() {
                        "r: resume managed task".into()
                    } else {
                        "Enter: read-only transcript".into()
                    },
                ]
            })
            .unwrap_or_default();
        let details = details
            .iter()
            .flat_map(|s| {
                s.split('\n')
                    .flat_map(|line| wrap(line, right.saturating_sub(2)))
            })
            .collect::<Vec<_>>();
        for n in 0..body {
            let (active, text) = list.get(start + n).cloned().unwrap_or_default();
            let text = padded(&text, left);
            let main = if active {
                format!("\x1b[7m{text}\x1b[0m")
            } else {
                text
            };
            lines.push(if right > 0 {
                format!(
                    "{main}│ {}",
                    details.get(n).map(String::as_str).unwrap_or("")
                )
            } else {
                main
            });
        }
    }
    while lines.len() < height.saturating_sub(1) {
        lines.push(String::new());
    }
    if height >= 4 {
        lines.push(fit(
            if detail.is_some() {
                "Esc back · ↑/↓ PgUp/PgDn scroll"
            } else {
                "? help · Esc back · ↑/↓ move · Tab filter · Enter read · r resume · n compose"
            },
            width,
        ));
    }
    lines.truncate(height);
    lines.join("\n")
}
fn detail_scroll_limit(text: &str, size: (usize, usize)) -> usize {
    text.lines()
        .flat_map(|line| wrap(line, size.0))
        .count()
        .saturating_sub(size.1.saturating_sub(4))
}
fn wrap(line: &str, width: usize) -> Vec<String> {
    if width == 0 {
        return Vec::new();
    }
    let mut out = Vec::new();
    let mut part = String::new();
    let mut used = 0;
    for ch in line.chars().filter(|c| !c.is_control()) {
        let w = ch.width().unwrap_or(0);
        if w > width {
            continue;
        }
        if used + w > width {
            out.push(std::mem::take(&mut part));
            used = 0;
        }
        part.push(ch);
        used += w;
    }
    out.push(part);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    fn task(id: &str, project: &str, state: TaskStatus) -> Task {
        Task {
            id: id.into(),
            project_key: project.into(),
            project: project.into(),
            title: format!("Task {id}"),
            status: state,
            updated: "1m ago".into(),
            model: "—".into(),
            reasoning: "—".into(),
            prompt: "Inspect task".into(),
            details: "Recorded transcript".into(),
            resume_handle: None,
        }
    }
    #[test]
    fn filters_real_states_and_groups_projects_without_losing_tasks() {
        let tasks = vec![
            task("a", "project-b", TaskStatus::Working),
            task("b", "project-a", TaskStatus::NeedsYou),
            task("c", "project-b", TaskStatus::Ready),
        ];
        assert_eq!(
            filtered(&tasks, 0)
                .iter()
                .map(|t| t.id.as_str())
                .collect::<Vec<_>>(),
            ["b", "a", "c"]
        );
        assert_eq!(filtered(&tasks, 1)[0].id, "b");
        assert_eq!(filtered(&tasks, 2)[0].id, "a");
        assert_eq!(filtered(&tasks, 3)[0].id, "c");
        assert!(filtered(&tasks, 4).is_empty());
    }
    #[test]
    fn same_named_projects_keep_separate_groups_and_counts() {
        let mut tasks = vec![
            task("a1", "app", TaskStatus::Ready),
            task("b1", "app", TaskStatus::Working),
            task("a2", "app", TaskStatus::Ready),
        ];
        tasks[0].project_key = "/work/a/app".into();
        tasks[1].project_key = "/work/b/app".into();
        tasks[2].project_key = "/work/a/app".into();
        assert_eq!(
            filtered(&tasks, 0)
                .iter()
                .map(|t| t.id.as_str())
                .collect::<Vec<_>>(),
            ["a1", "a2", "b1"]
        );
        let frame = render(&tasks, 0, 0, (80, 24), None, 0, "");
        assert!(frame.contains("app  2"));
        assert!(frame.contains("app  1"));
        assert!(!frame.contains("app  3"));
    }
    #[test]
    fn wide_details_and_narrow_transcript_use_real_selected_task() {
        let tasks = vec![task("a", "project", TaskStatus::Working)];
        let wide = render(&tasks, 0, 0, (180, 40), None, 0, "");
        assert!(wide.contains("Task details"));
        assert!(wide.contains("Inspect task"));
        assert!(wide.contains("Enter: read-only transcript"));
        let small = render(&tasks, 0, 0, (80, 24), None, 0, "");
        assert!(!small.contains("Task details"));
        assert!(small.contains("Task a"));
        assert_eq!(small.lines().count(), 24);
        let opened = render(&tasks, 0, 0, (80, 24), Some("Recorded transcript"), 0, "");
        assert!(opened.contains("Recorded transcript"));
    }
    #[test]
    fn empty_error_and_unicode_never_overflow_viewport() {
        for (w, h) in [(180, 40), (80, 24), (30, 8), (1, 1)] {
            let rendered = render(
                &[],
                0,
                0,
                (w, h),
                Some("中文任务\nMore text"),
                usize::MAX,
                "",
            );
            assert!(rendered.lines().count() <= h);
            assert!(rendered.lines().skip(1).all(|l| l.width() <= w));
        }
        assert!(render(&[], 0, 0, (80, 24), None, 0, "Cannot read source")
            .contains("Cannot read source"));
    }
    #[test]
    fn empty_filter_and_detail_scroll_limits_stay_explicit() {
        assert!(render(&[], 1, 0, (180, 40), None, 0, "").contains("No tasks in this filter."));
        assert!(render(&[], 4, 0, (30, 24), None, 0, "").contains("[Inactive 0]"));
        let text = (0..100)
            .map(|i| format!("Line {i}"))
            .collect::<Vec<_>>()
            .join("\n");
        let end = detail_scroll_limit(&text, (80, 24));
        assert_eq!(end, 80);
        assert!(render(&[], 0, 0, (80, 24), Some(&text), usize::MAX, "").contains("Line 80"));
        assert!(
            render(&[], 0, 0, (80, 24), Some(&text), end.saturating_sub(1), "").contains("Line 79")
        );
    }
}
