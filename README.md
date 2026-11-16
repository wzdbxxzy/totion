# Totion

Totion 是一个运行在终端里的共享空间，把 **计划 / 习惯 / 待办 / 日历 / 文档** 放在一起，两个人在同一个空间里协作编辑，并实时看到对方的改动。

```
┌─────────────┐    HTTPS/HTTP     ┌──────────────────┐
│  totion TUI │ ────────────────▶ │  totion-server   │
│  (Windows / │   Ed25519 登录     │  Rust + axum     │
│   macOS /   │   增量同步 2s      │  SQLite + change │
│   Linux)    │ ◀──────────────── │  log             │
└─────────────┘                   └──────────────────┘
        ▲                                   │
        └────────── 另一台客户端 ────────────┘
```

## 功能

- **Notion 式页面 + 内容块**：页面按「文档 / 计划 / 习惯 / 待办 / 日历」分组，内容块支持文本、标题、待办、习惯、日历事件、分割线。
- **共享空间**：服务器保存全部数据，多个用户用邀请码加入同一个空间。
- **免密登录**：客户端首次生成 Ed25519 密钥对，之后每次启动通过挑战-应答签名自动登录，不传输密码。
- **增量同步**：服务器记录全局变更日志（`seq`），客户端每 2 秒拉取 `seq > last_seq` 的变更；状态栏会显示「某某更新了……」。
- **乐观锁**：页面/内容块带版本号，编辑冲突时返回 `409` 并自动刷新为最新版本，避免静默覆盖。
- **习惯打卡**：记录连续天数（昨天打过卡则 +1，断签从 1 重新开始）。
- **日历视图**：月视图标记有事项的日期，列出本月日程。

> 当前协作粒度是「内容块级最后写入 + 版本冲突提示」，不是字符级 CRDT。两个人同时改**同一个块**时，后提交者会收到冲突并看到最新内容。

## 仓库结构

```
crates/
  totion-core/     共享类型、请求/响应协议、习惯打卡规则
  totion-server/   axum HTTP 服务 + SQLite + 变更日志
  totion-client/   ratatui TUI 客户端（二进制名 totion）
.github/workflows/ ci.yml（检查）、release.yml（三平台 Release + 部署）
deploy/totion.service  systemd 单元模板
```

## 本地开发

需要 Rust stable（1.85+）。

```bash
cargo test --workspace
cargo run -p totion-server -- --data-dir ./dev-data --bind 127.0.0.1:8080
cargo run -p totion-client
```

首次启动服务端会生成邀请码（同时写入 `dev-data/invite_code`），客户端首次运行填入服务器地址、名字和邀请码即可。

## 服务器部署（GaryServer）

CI 在打 tag 时自动完成：构建 Linux 二进制 → scp 到服务器 → 写 systemd 单元 → 重启 → 健康检查。

服务器上目录布局：

```
~/totion/
  totion-server     由 CI 更新
  totion.env        环境变量（首次部署生成）
  data/             SQLite、邀请码
```

查看邀请码：

```bash
ssh GaryServer
cat ~/totion/data/invite_code
journalctl -u totion -n 50
```

常用运维：

```bash
sudo systemctl status totion
sudo systemctl restart totion
journalctl -u totion -f
curl -s http://127.0.0.1:8080/api/health
```

## GitHub 配置

在仓库 `Settings → Secrets and variables → Actions` 中配置：

| Secret | 说明 |
|--------|------|
| `DEPLOY_HOST` | `64.31.25.155` |
| `DEPLOY_USER` | `admin` |
| `DEPLOY_SSH_KEY` | 部署专用私钥（对应公钥已写入服务器 `authorized_keys`） |
| `TOTION_INVITE_CODE` | 可选。固定邀请码；不设置则服务端随机生成 |

## 发版

```bash
git tag v0.1.0
git push origin v0.1.0
```

Release workflow 会产出：

- `totion-x86_64-unknown-linux-gnu.tar.gz`（客户端 + 服务端）
- `totion-aarch64-apple-darwin.tar.gz`（Apple Silicon）
- `totion-x86_64-apple-darwin.tar.gz`（Intel Mac）
- `totion-x86_64-pc-windows-msvc.zip`
- `SHA256SUMS.txt`

同时服务端自动部署到 GaryServer。

## 安全说明

- 登录采用 Ed25519 挑战-应答，服务器只保存公钥与会话 token（30 天）。
- 当前演示部署使用 HTTP。公网使用建议在前方加 TLS（Caddy/Nginx + 域名），否则数据与 token 明文传输。
- `totion.env` 权限 600；SQLite 数据库在 `~/totion/data/`，注意备份。
