## 1. Implementation
- [x] 1.1 Add API endpoint for remove-worktree on task attempts.
- [x] 1.2 Implement backend handler to validate eligibility, stop processes, and remove worktree data while clearing container_ref.
- [x] 1.3 Add frontend API call and confirmation dialog warning about data loss.
- [x] 1.4 Wire the action into the Attempt actions menu (attempt view) with visibility/disabled rules.
- [x] 1.5 Wire the action into the Task actions menu with attempt selection (default latest; no selection UI when only one).
- [x] 1.6 Add tests for backend removal behavior and frontend dialog state/validation. (Deferred per request; QA run only.)
