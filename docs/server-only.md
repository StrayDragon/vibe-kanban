# Server-only mode

The default `server` build embeds and serves the frontend UI from the backend binary.

Server-only mode is intended for multi-node deployments where the frontend is served
separately and backend nodes only need to provide APIs and MCP.

## Behavior

- `/api/**` and `/health` work normally.
- UI routes (`/`, `/{*path}`) return **404**.
- The build does not require `frontend/dist` to exist.

## Build / Run

Disable the embedded frontend feature on the `server` crate:

```bash
cargo build -p server --no-default-features
# or
cargo run -p server --no-default-features
```

Default (embedded UI) build:

```bash
cargo build -p server
```

