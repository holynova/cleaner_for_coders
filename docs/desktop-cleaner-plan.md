# 程序员专用空间清理工具 - 需求与开发方案（V1）

## 1. 需求确认（已对齐）

### 1.1 目标平台
- 支持 `macOS` 与 `Windows`

### 1.2 扫描能力
- 默认全盘扫描
- 提供高级选项，允许用户手动指定扫描目录
- 扫描对象（V1）：
  - 代码仓库中的 `node_modules`
  - `npm / pnpm / yarn` 缓存目录
  - 包管理工具全局缓存目录
  - Docker 可清理项（可配置范围）

### 1.3 排序与展示
- 支持按占用大小排序
- 支持按最近使用时间排序

### 1.4 清理策略
- `node_modules` 支持两种策略，用户可选：
  - 仅清理“较久未使用”
  - 全部清理
- Docker 清理范围支持多选：
  - `dangling images`（默认选中）
  - `unused images`
  - `stopped containers`
  - `build cache`
  - `volumes`
- 清理前需要二次确认与风险提示

### 1.5 可观测性
- 需要清理前可释放空间预估
- 清理时实时日志展示
- 清理后输出结果统计

### 1.6 高级功能
- 支持白名单/忽略规则（高级选项，默认折叠）
- 首版本不做定时扫描/自动清理
- UI 支持中英双语

---

## 2. 技术方案（已选）

- 框架：`Tauri 2 + Rust + React + TypeScript`
- 方案理由：
  - 桌面端性能较好，扫描与清理逻辑可由 Rust 提供稳定能力
  - 前端交互开发效率高，适合做复杂列表和日志界面
  - 可兼容 macOS / Windows

---

## 3. V1 功能边界

### 3.1 包含
- 本地磁盘扫描（node_modules + 包管理缓存）
- Docker 可清理项扫描（通过 Docker CLI）
- 结果列表、排序、勾选
- 清理执行 + 实时日志
- 中英双语切换
- 高级选项（路径、忽略规则、Docker 范围）

### 3.2 暂不包含
- 自动清理/定时任务
- 增量后台守护进程
- 云端配置同步

---

## 4. 实现设计

### 4.1 Rust 后端（Tauri Commands）
- `scan_cleanup_targets(payload)`
  - 输入：扫描路径、忽略规则、排序方式、node_modules 策略、Docker 选项
  - 输出：候选项列表（路径、类型、大小、最近使用时间、风险说明）与总可释放空间
- `execute_cleanup(payload)`
  - 输入：用户勾选项 + 执行策略
  - 输出：清理结果统计
  - 通过 Tauri `event` 推送实时日志

### 4.2 React 前端
- 筛选/排序控制区
- 扫描结果表格（多选）
- 预估空间统计卡片
- 清理确认弹窗（风险提示）
- 日志面板（实时滚动）
- 中英切换按钮
- 高级设置折叠区域

### 4.3 数据模型（核心字段）
- `CleanupItem`
  - `id`, `kind`, `path`, `size_bytes`, `last_used_unix`, `risk`, `source`
- `ScanSummary`
  - `total_bytes`, `item_count`
- `CleanupResult`
  - `freed_bytes`, `success_count`, `failed_count`

---

## 5. 里程碑

1. 初始化 Tauri 项目并建立基础 UI
2. 完成本地扫描逻辑（node_modules + caches）
3. 接入 Docker 扫描与清理
4. 接入实时日志流与确认弹窗
5. 双语与高级选项完善
6. 自测与交付说明

---

## 6. 风险与注意事项

- Docker 清理命令需要本机已安装并可访问 Docker CLI
- 部分目录可能需要管理员权限
- 删除 `node_modules` 后下次构建需重新安装依赖
- Windows 与 macOS 路径差异需要在代码层做兼容

---

## 7. 开发开始条件

- 需求已确认
- 方案已确认为 `Tauri + Rust + React`
- 进入编码阶段（本文件落地后立即执行）
