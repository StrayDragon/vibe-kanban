# Task: T-001 Auth Boundary for HTTP API

## Background / Motivation
- Issue: P0-SEC-01
- Evidence: No auth middleware and /api routes are open for LAN/public use.

## Scope
### In Scope
- Add AccessControlConfig to config schema.
- Add HTTP auth middleware and apply to /api routes.
- Redact sensitive token fields in UserSystemInfo responses.
- Inject token header for frontend fetch requests.

### Out of Scope / Right Boundary
- User accounts, OAuth, RBAC, or session login.
- Multi-tenant identity model.
- UI login flow.

## Design
### Proposed
- Config: access_control { mode, token, allow_localhost_bypass }
  - mode: disabled | token
  - allow_localhost_bypass default true
- Token sources: Authorization: Bearer <token> and X-API-Token.
- Apply middleware to /api router only; /health remains open.
- Redact token in UserSystemInfo responses (config clone with token removed).
- Frontend: makeRequest reads token from localStorage key vk_api_token.

### Alternatives Considered
- Cookie/session auth (deferred; requires login flow)
- Basic auth (too limited for future extensions)

### Trade-offs
- Shared-token model is simple and testable but not multi-user.

## Change List
- crates/services/src/services/config/schema.rs: add AccessControlConfig, bump config version, defaults.
- crates/services/src/services/config/mod.rs: export AccessControlConfig.
- crates/server/src/middleware/auth.rs: new middleware (token validation + localhost bypass).
- crates/server/src/routes/mod.rs: apply auth middleware to /api router.
- crates/server/src/routes/config.rs: return redacted config in UserSystemInfo.
- frontend/src/lib/api.ts: attach Authorization header if token exists.
- shared/types.ts: regenerate via pnpm run generate-types.

## Acceptance Criteria
- mode=disabled: API works without token.
- mode=token + allow_localhost_bypass=true: localhost requests succeed without token; non-localhost require token.
- mode=token + allow_localhost_bypass=false: requests without token return 401 + ApiResponse error.
- /health remains 200 without token.
- cargo test --workspace passes (new middleware tests included).

## Risks & Rollback
- Risk: misconfigured token locks out clients.
- Rollback: set mode=disabled or allow_localhost_bypass=true.

## Effort Estimate
- 1-2 days.

## Acceptance Scripts
### HTTP Auth Boundary
```bash
export BACKEND_PORT=3001

# 1) Fetch current config
curl -s "http://localhost:${BACKEND_PORT}/api/info" > /tmp/vk-info.json

# 2) Update config with access control (token mode)
python - <<'PY'
import json
info = json.load(open('/tmp/vk-info.json'))
config = info['data']['config']
config['accessControl'] = {
  'mode': 'TOKEN',
  'token': 'test-token',
  'allowLocalhostBypass': False
}
json.dump(config, open('/tmp/vk-config.json', 'w'))
PY

# 3) Save config
curl -i -X PUT "http://localhost:${BACKEND_PORT}/api/config" \\
  -H 'Content-Type: application/json' \\
  --data @/tmp/vk-config.json

# 4) /health remains open
curl -i "http://localhost:${BACKEND_PORT}/health"

# 5) /api/info rejects without token
curl -i "http://localhost:${BACKEND_PORT}/api/info"

# 6) /api/info accepts with token
curl -i -H "Authorization: Bearer test-token" \\
  "http://localhost:${BACKEND_PORT}/api/info"

# 7) Verify token is redacted in response
curl -s -H "Authorization: Bearer test-token" \\
  "http://localhost:${BACKEND_PORT}/api/info" > /tmp/vk-info-auth.json
python - <<'PY'
import json
info = json.load(open('/tmp/vk-info-auth.json'))
config = info['data']['config']
print('token' in config.get('accessControl', {}))
PY
```
Expected:
- Step 4 returns 200.
- Step 5 returns 401 + ApiResponse error.
- Step 6 returns 200.
- Step 7 prints False (token redacted).
