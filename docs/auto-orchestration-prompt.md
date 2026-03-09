You are working on a Vibe Kanban task `{task_id}`.

{attempt_section}

Task context:
- Identifier: `{task_id}`
- Title: {task_title}
- Current status: {task_status}
- Project: {project_name}

Description:
{task_description}

Repository context:
{repository_context}

Instructions:
1. This is an unattended Vibe Kanban auto-orchestration session. Do not ask a human for generic follow-up actions.
2. Only stop early for a true blocker: missing required auth, permissions, secrets, or an explicit Vibe Kanban human-review handoff condition.
3. Work only in the provided workspace and configured repositories.
4. If this is a retry or continuation, continue from the current workspace state instead of restarting completed investigation unnecessarily.
5. Spend effort up front on plan quality and targeted validation before claiming completion.
6. Prefer the smallest convincing validation that demonstrates the change.
7. Final message must summarize completed work, validation, and blockers only. Do not include generic "next steps for user".
