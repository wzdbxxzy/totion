# Totion

Totion 是一个运行在终端里的共享空间：**今日待办 / 日记、习惯坚持、日历日程、统计、评论**放在一起，两个人在同一个空间里协作，自动同步。

```
┌─────────────┐     HTTP(S)       ┌──────────────────┐
│  totion TUI │ ────────────────▶ │  totion-server   │
│ Windows /   │  Ed25519 免密登录  │  Rust + axum     │
│ macOS /     │  增量同步 2 秒     │  SQLite + change │
│ Linux       │ ◀──────────────── │  log             │
└─────────────┘                   └──────────────────┘
```

## 功能（v0.2）

- **今日**：当天待办/日程按时间排序、习惯打卡、自动创建的每日日记页（总结 / 感悟 / 经验），顶部显示完成率。
- **工作区**：Notion 式页面 + 内容块（文本、标题、待办、旧版习惯、日程、分割线）。
- **日历**：月历 + 选中日议程；`h/l` 选日、`j/k` 前后一周、`[` `]` 换月，可直接新建带 `HH:MM` 时间的待办/日程。
- **习惯**：独立于待办的坚持性任务，支持「每天 / 工作日 / 每周 N 次」；每次打卡独立记录，提供连续天数、最长连续、30 天完成率、12 周热力图。
- **统计**：今日完成率、最近 7/14/30/90 天完成柱状图与完成率趋势、习惯汇总热力图、每个习惯的统计表。
- **评论**：页面、内容块、习惯都能评论；显示作者和时间，可标记已解决、编辑、删除。
- **搜索**：`/` 搜索页面标题、内容块文本、习惯名称并跳转。
- **免密登录**：首次用邀请码 + Ed25519 密钥注册，之后挑战-应答自动登录。
- **增量同步**：服务器记录全局变更日志，客户端每 2 秒按游标拉取；块级乐观锁冲突返回 409 并自动刷新。

## 键位

| 键 | 作用 |
|----|------|
| `1` `2` `3` `4` `5` | 今日 / 工作区 / 日历 / 习惯 / 统计 |
| `Tab` | 工作区切换页面/内容面板 |
| `j` `k` / `↑` `↓` | 移动 |
| `h` `l` / `←` `→` | 日历选日、统计切换时间窗口 |
| `[` `]` | 日历换月 |
| `Enter` / `e` | 编辑 |
| `Space` | 勾选待办 / 习惯打卡 |
| `u` | 撤销今天的打卡 |
| `t` / `c` / `n` | 新建待办 / 日程 / 习惯（视上下文） |
| `N` | 新建页面 |
| `C` | 评论面板 |
| `/` | 搜索 |
| `?` | 帮助 |
| `r` | 手动同步 |
| `d` | 删除 |
| `q` / `Ctrl+C` | 退出 |

编辑：`Enter` 保存；日记/评论等多行内容用 `Ctrl+S` 保存，`Enter` 换行，`Esc` 取消。

评论面板：`a` 添加、`e` 编辑、`r` 解决/取消解决、`d` 删除。

## 仓库结构

```
crates/
  totion-core/     共享类型、协议、日期/统计纯函数
  totion-server/   axum HTTP 服务 + SQLite + 变更日志 + 迁移
  totion-client/   ratatui TUI（二进制 totion）
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

首次启动服务端生成邀请码（写入 `dev-data/invite_code`）。

> 本机（无 MSVC）使用 GNU 工具链：
> `export PATH="/c/Users/hasee/scoop/apps/mingw/current/bin:$PATH"`，然后
> `cargo +stable-x86_64-pc-windows-gnu run -p totion-client`。

## 服务器部署

CI 打 tag 后自动：构建 Linux 二进制 → scp 为 `totion-server.new` → 停服务 → 备份 SQLite → 替换二进制 → 安装 systemd 单元 → 启动 → 健康检查。

服务器目录：

```
~/totion/
  totion-server        当前二进制
  totion-server.new    部署暂存
  totion.env           环境变量（600）
  data/totion.db       SQLite
  data/totion.db.bak.* 部署前自动备份
  data/invite_code     邀请码
```

查看邀请码 / 日志：

```bash
ssh GaryServer
cat ~/totion/data/invite_code
journalctl -u totion -n 50
sudo systemctl status totion
```

## GitHub 配置

仓库 `Settings → Secrets and variables → Actions`：

| Secret | 说明 |
|--------|------|
| `DEPLOY_HOST` | `64.31.25.155` |
| `DEPLOY_USER` | `admin` |
| `DEPLOY_SSH_KEY` | 部署专用私钥 |
| `TOTION_INVITE_CODE` | 可选，固定邀请码 |

## 发版

```bash
git tag v0.2.0
git push origin v0.2.0
```

产物：Linux（客户端+服务端）、macOS arm64/x64、Windows x64，以及 `SHA256SUMS.txt`；同时自动部署服务端。

## 数据与兼容

- 数据库迁移由 `meta.schema_version` 驱动，启动时可重复执行；旧 Habit 块保留不自动迁移。
- 旧版 v0.1 客户端仍可读写页面/块；服务端按 `X-Totion-Protocol` 过滤新实体，不会把新数据当成旧块。
- 当前演示部署是明文 HTTP，公网建议加域名 + Caddy 自动 HTTPS（客户端把地址改成 `https://` 即可）。
