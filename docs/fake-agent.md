# Fake Agent Scenarios

This document explains how to run the fake-agent scenario used to validate image handling.

## Image Scenario
The repo includes an image-focused scenario at:
- `assets/fake-agent/image-scenario.jsonl`

## Configure a FakeAgent profile
1. Open your asset directory (`VIBE_ASSET_DIR`, see `docs/operations.md`).
2. Edit `profiles.json` and add/update a FakeAgent profile with a scenario path (prefer absolute paths):

```json
{
  "fake_agent": {
    "scenario_path": "/absolute/path/to/vibe-kanban/assets/fake-agent/image-scenario.jsonl"
  }
}
```

3. Select the FakeAgent executor in the UI and start an attempt.
4. The scenario emits image-related events that should render `.vibe-images/...` placeholders.

Note: the scenario uses a placeholder `.vibe-images/demo.png` path. If you want a real image preview, upload any image to a task so the markdown resolves to an existing image in your UI.
