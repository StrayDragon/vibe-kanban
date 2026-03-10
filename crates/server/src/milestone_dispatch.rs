use std::collections::HashMap;

use chrono::Utc;
use db::models::{
    milestone::{Milestone, MilestoneEdge, MilestoneNodeKind},
    task::{TaskStatus, TaskWithAttemptStatus},
};
use uuid::Uuid;

pub fn milestone_dispatch_enabled(milestone: &Milestone) -> bool {
    milestone.automation_mode == db::types::MilestoneAutomationMode::Auto
        || milestone.run_next_step_requested_at.is_some()
}

pub fn milestone_has_active_attempt(
    milestone: &Milestone,
    tasks_by_id: &HashMap<Uuid, TaskWithAttemptStatus>,
) -> bool {
    let now = Utc::now();
    milestone.graph.nodes.iter().any(|node| {
        tasks_by_id.get(&node.task_id).is_some_and(|task| {
            if task.has_in_progress_attempt {
                return true;
            }
            task.dispatch_state.as_ref().is_some_and(|state| {
                state.status == db::types::TaskDispatchStatus::Claimed
                    && state
                        .claim_expires_at
                        .map(|expires_at| expires_at > now)
                        .unwrap_or(true)
            })
        })
    })
}

pub fn next_milestone_dispatch_candidate<'a>(
    milestone: &Milestone,
    tasks_by_id: &'a HashMap<Uuid, TaskWithAttemptStatus>,
) -> Option<&'a TaskWithAttemptStatus> {
    let mut status_by_node_id: HashMap<&str, TaskStatus> =
        HashMap::with_capacity(milestone.graph.nodes.len());
    for node in &milestone.graph.nodes {
        let key = node.id.trim();
        if key.is_empty() {
            continue;
        }
        status_by_node_id.insert(key, node.status.clone().unwrap_or(TaskStatus::Todo));
    }

    let mut candidates = Vec::new();
    for node in &milestone.graph.nodes {
        let node_id = node.id.trim();
        if node_id.is_empty() {
            continue;
        }
        if matches!(&node.kind, MilestoneNodeKind::Checkpoint)
            || node.requires_approval.unwrap_or(false)
        {
            continue;
        }
        if !predecessors_done(&milestone.graph.edges, &status_by_node_id, node_id) {
            continue;
        }

        let Some(task) = tasks_by_id.get(&node.task_id) else {
            continue;
        };
        if task.task_kind == db::types::TaskKind::Milestone {
            continue;
        }
        if task.milestone_id != Some(milestone.id) {
            continue;
        }
        if task
            .milestone_node_id
            .as_deref()
            .map(str::trim)
            != Some(node_id)
        {
            continue;
        }
        if !task_dispatch_candidate(task) || !retry_ready(task) {
            continue;
        }

        candidates.push(node);
    }

    candidates.sort_by(|a, b| a.phase.cmp(&b.phase).then_with(|| a.id.cmp(&b.id)));
    candidates
        .first()
        .and_then(|node| tasks_by_id.get(&node.task_id))
}

fn predecessors_done(
    edges: &[MilestoneEdge],
    status_by_node_id: &HashMap<&str, TaskStatus>,
    node_id: &str,
) -> bool {
    let node_id = node_id.trim();
    if node_id.is_empty() {
        return false;
    }

    for edge in edges {
        if edge.to.trim() != node_id {
            continue;
        }
        let from = edge.from.trim();
        if from.is_empty() {
            continue;
        }
        match status_by_node_id.get(from) {
            Some(TaskStatus::Done) => {}
            _ => return false,
        }
    }

    true
}

fn task_dispatch_candidate(task: &TaskWithAttemptStatus) -> bool {
    if task.has_in_progress_attempt {
        return false;
    }
    if matches!(task.status, TaskStatus::Done | TaskStatus::Cancelled) {
        return false;
    }
    if let Some(state) = &task.dispatch_state {
        if state.status == db::types::TaskDispatchStatus::Blocked {
            return false;
        }
        if state.status == db::types::TaskDispatchStatus::Claimed
            && state
                .claim_expires_at
                .map(|expires_at| expires_at > Utc::now())
                .unwrap_or(true)
        {
            return false;
        }
    }

    match task.status {
        TaskStatus::Todo => true,
        TaskStatus::InReview => task.last_attempt_failed,
        _ => false,
    }
}

fn retry_ready(task: &TaskWithAttemptStatus) -> bool {
    let Some(state) = task.dispatch_state.as_ref() else {
        return true;
    };

    if state.status != db::types::TaskDispatchStatus::RetryScheduled {
        return true;
    }

    state
        .next_retry_at
        .map(|next_retry_at| next_retry_at <= Utc::now())
        .unwrap_or(true)
}
