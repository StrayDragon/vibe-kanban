## Context

当前 Settings 页面由 `frontend/src/pages/settings/SettingsLayout.tsx` 组织为三个 section：

- Config：`frontend/src/pages/settings/GeneralSettings/GeneralSettings.tsx`
- Projects：`frontend/src/pages/settings/ProjectSettings/ProjectSettings.tsx`
- MCP：`frontend/src/pages/settings/McpSettings.tsx`

其中 Config 区域包含较多“路径/命令 + Copy + hint”的纵向罗列（多个 `space-y-*` 的 key-value 块），在信息密度、对齐一致性和扫视效率上存在改进空间。Projects 的 “Configured projects” 目前使用卡片列表展示，项目数量较多时占用纵向空间较大。

前端已存在可复用的表格基础组件：

- `frontend/src/components/ui/table/table.tsx`：Table primitives
- `frontend/src/components/ui/table/data-table.tsx`：轻量 DataTable
- `frontend/src/components/TagManager.tsx`：已有 sticky header + 可滚动 table 的实现参考

该变更仅涉及前端展示与交互细节，不改变任何后端接口与配置语义；Settings 保持只读（UI 不写入 `config.yaml` / `projects.yaml` / `secret.env`）。

## Goals / Non-Goals

**Goals:**

- 将 Config 与 Projects 的“只读信息展示”改为更易扫视的表格布局，提升信息密度与对齐一致性。
- 对长路径/命令提供稳定的展示策略（换行或横向滚动），避免破版与操作区域被挤压。
- 保持可访问性：表头语义明确，Copy 操作可键盘访问，交互控件具备可读的 `title`/`aria-label`。
- 保持改动聚焦：不影响 Settings 的页面结构（Config / Projects / MCP 仍按原顺序）。

**Non-Goals:**

- 提供 UI 直接编辑配置文件的能力（继续保持只读 + Reload/Refresh 等现有能力）。
- 改动后端 API、配置 schema 或配置版本迁移逻辑（本变更不涉及 config versioning / migration）。
- 重做 MCP 展示结构（除非实现过程中出现明显复用机会，否则保持现状）。

## Decisions

### 1) Config：采用“Key-Value 表格”替代纵向罗列

**Decision:** 将 Config 区域的主要信息（Loaded at、config dir、config.yaml、projects.yaml、projects.d、secret.env、schema 相关路径与命令等）统一渲染为表格（建议 3 列：Item / Value / Actions）。

**Rationale:**

- 表格能提供稳定的列对齐与更高的信息密度，适合展示大量只读的键值信息。
- Actions 列可统一为 icon 按钮，显著降低重复“Copy”文字带来的视觉噪音。

**Alternatives considered:**

- 继续使用纵向 key-value，但用 CSS grid 对齐：对齐能改善，但仍会产生大量重复按钮与上下间距冗余；且 hint 文案与 value 的关系不如表格直观。

### 2) Table 组件选型：优先复用现有 `ui/table` primitives，必要时引入轻量 wrapper

**Decision:** 基于 `frontend/src/components/ui/table/table.tsx` 的 primitives 实现 Settings 专用表格布局（可在 Settings 目录内封装一个小组件，或在页面内直接使用 primitives）。

**Rationale:**

- primitives 足够轻量，便于实现多行 Value（value + hint）以及对 Actions 列的固定宽度控制。
- 避免对通用 `DataTable` 做侵入式改动（例如 sticky header、额外 className 注入）带来的全局影响。

**Alternatives considered:**

- 直接使用 `DataTable`：可行但可能需要扩展 API（例如 head/body container、sticky header、样式注入），会影响现有用例与维护边界。

### 3) Projects：Configured projects 由卡片列表改为表格

**Decision:** 将 “Configured projects” 区域改为表格展示（至少包含 Name 与 ID；可选提供 Copy ID）。

**Rationale:**

- 项目列表天然适合表格；更节省纵向空间且便于对比。
- 保留现有 Loading / Empty / Error 状态语义，仅替换展示方式。

### 4) 文案与可访问性策略

**Decision:** 表头文案尽量复用现有 i18n key；缺失时新增少量 key（例如 Item/Value/Actions、Project name/ID 等）。Copy 操作使用 icon 按钮，但必须提供 `title` 与 `aria-label`。

## Risks / Trade-offs

- **[Risk]** 长路径/命令导致移动端拥挤 → **Mitigation**：Value 列采用 `break-all`/`whitespace-pre-wrap` 与 `overflow-x-auto` 的组合；Actions 列固定宽度并保持可点击区域。
- **[Risk]** i18n 文案长度差异影响列宽/对齐 → **Mitigation**：为 label 列设置合理的 `min-w-*`/`w-*`，Value 列自适应，Actions 列固定。
- **[Trade-off]** icon-only Copy 降低可发现性 → **Mitigation**：保留图标 + hover 提示（`title`），必要时在窄屏显示简短文字。

## Migration Plan

- 本变更为纯前端 UI 展示调整：
  - 无数据库变更
  - 无 API 变更
  - 无配置文件语义变更
- 回滚策略：如出现布局问题，可回退相关前端提交；不涉及数据迁移与兼容层。

## Open Questions

- MCP 区域是否也需要统一表格化（目前代码片段展示结构较清晰，暂不强制）。
- Config 表格是否需要 sticky header（条目数量固定较少，更多是视觉一致性需求，可在实现时权衡）。
