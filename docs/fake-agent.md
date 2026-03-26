# Fake Agent Scenarios

This document explains how to run the fake-agent scenario used to validate image handling.

## Image Scenario
The repo includes an image-focused scenario at:
- `assets/fake-agent/image-scenario.jsonl`

## Configure a FakeAgent profile
1. Ensure Fake Agent support is enabled in your build:
   - `cargo build -p executors --features fake-agent --bin fake-agent`
   - Run the server with `cargo run -p server --bin server --features executors/fake-agent` (dev) or an equivalent build flag.
2. Edit your user config (`VK_CONFIG_DIR` override, default Linux/macOS: `~/.config/vk/config.yaml`) and add a FakeAgent profile override with a scenario path (prefer absolute paths):

```yaml
executor_profile:
  executor: FAKE_AGENT

executor_profiles:
  executors:
    FAKE_AGENT:
      DEFAULT:
        FAKE_AGENT:
          scenario_path: /absolute/path/to/vibe-kanban/assets/fake-agent/image-scenario.jsonl
```

3. Reload config (`POST /api/config/reload`) and start an attempt using the FakeAgent profile.
4. The scenario emits image-related events that should render `.vibe-images/...` placeholders.

Note: the scenario uses a placeholder `.vibe-images/demo.png` path. If you want a real image preview, upload any image to a task so the markdown resolves to an existing image in your UI.
