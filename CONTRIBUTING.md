# 贡献指南

感谢你愿意改进 totion。

## 开发环境

- Rust stable 1.85+
- Windows / macOS / Linux 均可
- 无需数据库服务，SQLite 由 `rusqlite` 内嵌

```bash
cargo run -p totion-server -- --data-dir ./dev-data --bind 127.0.0.1:8080
cargo run -p totion-client
```

首次启动服务端后，邀请码在 `dev-data/invite_code`。

## 提交前检查

```bash
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

CI 会执行同样的三项检查。

## 代码约定

- 共享类型、协议、纯计算放 `totion-core`，不要在客户端/服务端各写一份。
- 新实体接入同步时，必须同时修改：
  - `totion_core::EntityKind`（`as_wire` / `from_wire`）
  - 服务端 `db.rs` 的读写函数与 `api.rs` 路由
  - 客户端 `apply_change` 与 `upsert_*`
- 写操作必须走 `*_and_change` 事务函数，保证实体与变更日志一致。
- 数据库变更只通过 `db::migrate` 的版本迁移，不直接改已有表。
- 面向用户的文案使用中文；代码标识符、注释和提交信息使用英文。

## 分支与提交

- 从 `main` 拉分支：`feat/xxx`、`fix/xxx`。
- 提交信息建议使用 Conventional Commits：`feat: ...`、`fix: ...`、`docs: ...`。
- PR 描述里写清楚：动机、改动点、验证方式（命令或截图）。

## 报告问题

请附上：

- 操作系统与版本
- 客户端/服务端版本（`totion --version` 或 Release 标签）
- 复现步骤与相关日志（`journalctl -u totion -n 100`）

安全问题请勿公开提交 issue，直接联系维护者。
