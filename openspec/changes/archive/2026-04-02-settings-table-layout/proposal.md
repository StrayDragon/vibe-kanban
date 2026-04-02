## Why

当前设置页面（Config / Projects）主要以纵向罗列的 key-value 区块呈现：路径/命令较长时对齐不稳定，信息密度低，难以快速扫视定位；同时每行重复的 Copy 按钮带来较多视觉噪音。将其改为表格布局可以显著提升可读性、一致性与可访问性。

## What Changes

- 将 Settings > Config 中的配置路径/命令/提示信息由“逐条罗列”改为统一表格展示（例如 3 列：Item / Value / Actions）。
- 将 Settings > Projects 中的 “Configured projects” 由卡片列表改为表格展示（例如 Name / ID / Actions）。
- 复用现有 `frontend/src/components/ui/table` 组件体系（或新增轻量 wrapper）以保持风格一致，并强化长文本处理与响应式滚动。
- 不改变任何后端 API、数据模型与配置语义；仅改善前端展示与交互细节。

## Capabilities

### New Capabilities

- `settings-table-layout`: Settings 页面以表格形式展示只读配置状态与项目列表（包含复制动作、长文本换行/滚动、表头语义与键盘可达等表现约束）。

### Modified Capabilities

- （无）

## Impact

- Frontend
  - `frontend/src/pages/settings/GeneralSettings/GeneralSettings.tsx`
  - `frontend/src/pages/settings/ProjectSettings/ProjectSettings.tsx`
  - 可能新增/扩展 `frontend/src/components/ui/table/*` 或新增 Settings 专用展示组件
- i18n：可能补充少量表头/操作文案（优先复用现有 key，避免破坏性变更）

## Goals

- 信息密度更高，支持快速扫视定位（尤其是路径/命令类内容）。
- 视觉一致：统一列宽、对齐、Action 区域布局，降低重复按钮带来的噪音。
- 可访问性：保留明确的表头语义，Copy/操作可通过键盘访问。
- 响应式：窄屏允许横向滚动，长文本不破版。

## Non-goals

- 通过 UI 编辑/写入 `config.yaml` / `projects.yaml` / `secret.env`（继续保持只读）。
- 重构 Settings 的信息架构或新增复杂导航（仅做当前页面呈现优化）。
- 改动后端/配置加载逻辑。

## Risks

- 路径/命令过长导致表格在移动端拥挤：需要 `overflow-x-auto` 与 `break-all`/`truncate` 的平衡。
- i18n 文案长度差异导致列宽抖动：需要合理的 `min-width`/`w-*` 策略。
- Copy 从文字按钮调整为 icon 按钮可能降低可发现性：需要 `title`/tooltip/aria-label。

## Verification

- 手动：打开 `/settings`，检查 Config 表格对齐、Copy 正常、提示信息可读；Projects 表格在项目较多时滚动与对齐正常。
- 键盘：Tab 可聚焦到 Copy 按钮；ESC 仍可关闭设置页（现有行为不变）。
- 主题/语言：在暗色/亮色、英文/中文下检查布局不溢出。
- 工程：`pnpm run check`、`pnpm run lint`（必要时补充相关测试/快照）。
