## 1. 表格基础与文案

- [x] 1.1 选择实现路径（`ui/table` primitives + Settings 专用 wrapper 或直接页面内使用），并落地最小可复用表格容器（带边框/滚动容器/表头样式）——验证：`pnpm run check` 通过
- [x] 1.2 如需新增表头/操作文案 i18n key（Item/Value/Actions、Name/ID 等），补齐 EN + zh-Hans ——验证：切换语言后 `/settings` 无缺失文案

## 2. Config（GeneralSettings）表格化

- [x] 2.1 将 `frontend/src/pages/settings/GeneralSettings/GeneralSettings.tsx` 中的 key-value 罗列改为“行数据 + 表格渲染”（Item/Value/Actions）——验证：打开 `/settings#config`，表格对齐稳定
- [x] 2.2 保持现有按钮与状态逻辑不变（Reload/Refresh、dirty/last_error/read-only alerts）——验证：手动点击 Reload/Refresh，页面行为与改动前一致
- [x] 2.3 为每个可复制项提供 copy action（icon 按钮 + `title`/`aria-label`），确保复制的是完整底层值（非截断展示）——验证：复制 `config.yaml` 路径与 `vk config schema upsert` 后粘贴内容完整
- [x] 2.4 处理长路径/命令的窄屏可用性（避免挤压 Actions 列，必要时横向滚动或换行策略）——验证：将窗口缩窄后仍可操作 Copy 且内容不破版

## 3. Projects（ProjectSettings）表格化

- [x] 3.1 将 “Configured projects” 从卡片列表替换为表格（至少 Name/ID 两列）——验证：打开 `/settings#projects`，项目多时可快速扫视
- [x] 3.2 保持现有 Loading / Empty / Error 语义与文案 ——验证：在断开/加载/无项目时 UI 状态正确显示
- [x] 3.3 （可选）为项目 ID 增加 Copy action，并保证键盘可达 ——验证：Tab 可聚焦到按钮，复制后粘贴为完整项目 ID

## 4. 验证与收尾

- [x] 4.1 运行前端检查与格式化相关命令（至少 `pnpm run check`、`pnpm run lint`）——验证：命令全绿
- [x] 4.2 手动 smoke test：`/settings` 在 EN/zh-Hans、暗色/亮色下检查表格布局、滚动与可访问性（Tab/hover 提示）——验证：无明显溢出/错位/不可操作控件
