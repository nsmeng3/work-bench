# WorkBench App

Tauri 2 桌面应用：React 19 + TypeScript + Vite 前端，Rust 后端，SQLite（sqlx）持久化。

## 开发

```bash
pnpm install
pnpm tauri dev
```

要求：Node ≥ 24、pnpm ≥ 10、Rust stable、Tauri CLI 2.x。

## 目录结构

```
app/
├── src/                前端（React + antd）
│   ├── pages/          页面：Dashboard / Inbox / Collection / Space / Filter / Todo / Terminal / Stats / Audit / Settings
│   ├── components/     业务组件
│   ├── hooks/          自定义 hooks
│   ├── api/            Tauri command 封装
│   └── float/          悬浮通知窗（独立窗口入口）
└── src-tauri/
    └── src/            Rust 后端：watch（目录监听）/ aggregate / inbox / collection / space / todo / terminal / tray / float 等
```

## 测试

```bash
# 前端类型检查 + 构建
pnpm build

# Rust 测试（含 macOS FSEvents e2e）
cd src-tauri && cargo test
```

## 打包

```bash
pnpm tauri build
```

本地打包只出当前平台的产物；跨平台包走 GitHub Actions（见仓库根 README「发布」一节）。

## 桌面通知

应用内主动提醒统一走悬浮窗通道（`float_notify`），不要引入系统级通知。详见根目录 `CLAUDE.md`。
