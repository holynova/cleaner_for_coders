# Dev Space Cleaner | 程序员空间清理工具

A lightweight desktop cleaner for developers, built with `Tauri 2 + Rust + React`.
一个面向开发者的轻量级桌面清理工具，基于 `Tauri 2 + Rust + React`。

## Features | 功能

- Scan cleanup targets | 扫描清理对象
  - `node_modules` in projects | 项目中的 `node_modules`
  - `npm / pnpm / yarn` caches | `npm / pnpm / yarn` 缓存
  - Docker cleanup targets (optional) | Docker 可清理项（可选）
- Sort results by size or last used time | 按占用大小或最近使用时间排序
- Multi-select cleanup with confirmation | 多选清理并二次确认
- Live cleanup logs and summary report | 实时清理日志与结果统计
- Chinese / English UI switch | 中英文界面切换
- Advanced options: custom paths, ignore rules, Docker scope | 高级选项：自定义路径、忽略规则、Docker 范围

## Tech Stack | 技术栈

- Tauri 2
- Rust
- React + TypeScript
- Vite

## Quick Start | 快速开始

### Prerequisites | 环境要求

- Node.js 18+
- pnpm 9+
- Rust toolchain (`cargo`, `rustc`)
- Tauri platform prerequisites: <https://tauri.app/start/prerequisites/>

### Install dependencies | 安装依赖

```bash
pnpm install
```

### Run desktop app | 启动桌面端

```bash
pnpm tauri dev
```

### Build frontend | 构建前端

```bash
pnpm build
```

## Notes | 注意事项

- Removing `node_modules` means dependencies must be reinstalled later.
  删除 `node_modules` 后，需要在对应项目中重新安装依赖。
- Docker cleanup requires Docker CLI in PATH.
  Docker 清理依赖本机已安装并可访问 Docker CLI。
- Full-disk scan can be slow on large disks.
  全盘扫描在大磁盘设备上可能较慢。
