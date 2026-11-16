<p align="center">
  <img src="docs/banner.svg" alt="totion" width="100%">
</p>

<p align="center">
  <a href="https://github.com/wzdbxxzy/totion/actions/workflows/ci.yml"><img src="https://github.com/wzdbxxzy/totion/actions/workflows/ci.yml/badge.svg" alt="CI"></a>
  <a href="https://github.com/wzdbxxzy/totion/releases"><img src="https://img.shields.io/github/v/release/wzdbxxzy/totion?label=release" alt="Release"></a>
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-MIT-yellow.svg" alt="License: MIT"></a>
  <img src="https://img.shields.io/badge/rust-1.85%2B-orange?logo=rust" alt="Rust">
  <img src="https://img.shields.io/badge/platform-Windows%20%7C%20macOS%20%7C%20Linux-blue" alt="Platforms">
</p>

totion 是一个跑在终端里的共享清单工具。待办、习惯、日历、日记和评论放在同一个空间里，多人共用一份数据，每个客户端每 2 秒自动同步。

它由两部分组成：一个单文件的 TUI 客户端（Rust + ratatui），和一个自托管的同步服务（Rust + axum + SQLite）。没有账号系统，不依赖云服务，数据就是一个 SQLite 文件。

适合几个人一起用：一个共享空间、一个邀请码，谁都能加待办、打卡、写日记、留评论。技术上没有成员数量上限，只是目前按小规模使用设计，没有角色和权限区分。

## 安装

### 直接下载

从 [Releases](https://github.com/wzdbxxzy/totion/releases) 下载：

| 系统 | 文件 |
|------|------|
| Windows 10/11 x64 | `totion-x86_64-pc-windows-msvc.zip` |
| macOS Apple Silicon | `totion-aarch64-apple-darwin.tar.gz` |
| macOS Intel | `totion-x86_64-apple-darwin.tar.gz` |
| Linux x64 | `totion-x86_64-unknown-linux-gnu.tar.gz`（含客户端和服务端） |

Windows：

```powershell
Expand-Archive .\totion-x86_64-pc-windows-msvc.zip -DestinationPath .\totion
cd .\totion
.\totion.exe
```

macOS / Linux：

```bash
tar -xzf totion-*.tar.gz
./totion
```

第一次运行时，Windows 可能提示 SmartScreen，点“更多信息 → 仍要运行”；macOS 可能提示无法验证开发者，执行一次：

```bash
xattr -d com.apple.quarantine ./totion
```

### 从源码编译

需要 Rust 1.85 或更新的 stable 版本：

```bash
git clone git@github.com:wzdbxxzy/totion.git
cd totion
cargo build --release -p totion-client
```

编译出的程序在 `target/release/totion`（Windows 是 `totion.exe`）。

## 第一次使用

### 1. 准备一个服务器

如果已经有人给你了地址，直接跳到下一步。自己部署的话，在服务器上启动：

```bash
./totion-server --bind 0.0.0.0:8080 --data-dir ./data
```

第一次启动会生成邀请码，保存在 `data/invite_code`：

```bash
cat data/invite_code
```

以后部署、升级、开机自启见[部署](#部署)。

### 2. 启动客户端

```bash
./totion
```

第一次运行会进入连接界面：

<p align="center">
  <img src="docs/screenshots/setup.svg" alt="首次连接界面" width="100%">
</p>

- `Tab` 在三个输入框之间切换。
- 填服务器地址、你的名字、邀请码。
- `Enter` 连接。

连接成功后，客户端会生成一对 Ed25519 密钥，保存在：

- Windows：`%APPDATA%\totion\identity.json`
- macOS：`~/Library/Application Support/totion/identity.json`
- Linux：`~/.config/totion/identity.json`

以后启动会直接用私钥签名登录，不需要再填密码。如果密钥丢失，可以用 `Ctrl+R` 在连接界面重新生成，但重新生成后会以一个新成员加入。

### 3. 邀请其他人

把服务器地址和同一个邀请码发给其他人。新成员注册后，你们就在同一个空间里，各自的操作会互相同步。顶部会显示当前空间的成员数量。

### 4. 进入今日

连接后默认进入「今日」，并自动创建当天的日记页。如果当天第一次打开，会看到三个空的记录区：总结、感悟、经验。

<p align="center">
  <img src="docs/screenshots/today.svg" alt="今日界面" width="100%">
</p>

顶部是日期和完成率。中间左边是当天的待办和日程，按开始时间排序；右边是今天该做的习惯和本周进度。下面三个区域用来写当天的总结、感悟和经验。

## 界面说明

### 今日

- `t` 新建待办，会弹出表单：

  <p align="center">
    <img src="docs/screenshots/new-event.svg" alt="新建待办" width="100%">
  </p>

  填标题、日期、开始和结束时间（`HH:MM`），`Tab` 切换字段，`Enter` 保存。

- `c` 新建日程，表单和待办一样。
- `Space` 勾选待办；习惯上是打卡，再按一次不会重复打卡。
- `u` 撤销今天某个习惯的打卡。
- `Enter` 编辑当前选中的待办、日程或日记区域。
- `d` 删除。
- 日记三个区域支持多行：编辑时 `Enter` 换行，`Ctrl+S` 保存，`Esc` 取消。

新建的待办和日程会挂到当天的日记页，并带上日期和时间，所以日历和统计里也能看到。

### 工作区

<p align="center">
  <img src="docs/screenshots/workspace.svg" alt="工作区界面" width="100%">
</p>

左边是页面列表，按类型分组（文档、计划、习惯、待办、日历）。右边是选中页面的内容块。

- `j` / `k` 移动，`Tab` 在页面列表和内容块之间切换。
- `N` 新建页面。
- `n` 新建文本块，`t` 新建待办块，`c` 新建日程块。
- `Enter` 编辑页面标题或内容块。
- `Space` 勾选待办块。
- `d` 删除。
- `a` 显示或隐藏自动生成的日记页，默认隐藏，避免页面列表被每日日记占满。
- 顶部 `[数字]` 表示该页面或内容块有未解决的评论。

### 日历

<p align="center">
  <img src="docs/screenshots/calendar.svg" alt="日历界面" width="100%">
</p>

左边是月历，右边是选中那天的议程。

- `h` / `l` 选择前一天、后一天，`j` / `k` 选择前一周、后一周。
- `[` / `]` 切换月份。
- `c` 在选中日期新建日程，`t` 新建待办。
- `Space` 勾选议程里的待办或日程。
- `Enter` 编辑，`d` 删除。
- 有安排的日期会显示绿点，今天和选中的日期有不同底色。

在日历上新建内容时，会自动为那天创建日记页（如果还没有的话），所以可以直接给未来某天安排事情。

### 习惯

<p align="center">
  <img src="docs/screenshots/habits.svg" alt="习惯界面" width="100%">
</p>

左边是习惯列表，右边是选中习惯的详情。

- `n` 新建习惯。表单里按 `Ctrl+F` 在“每天 / 工作日 / 每周 N 次”之间切换。
- `Space` 今天打卡，`u` 撤销今天的打卡。
- `e` 修改习惯名称，`d` 删除。
- 右侧显示连续天数、最长连续、30 天完成率和最近 12 周的热力图。

习惯和待办是分开的：待办通常只做一次，习惯是可以持续打卡的。每次打卡都会单独记录，所以可以看历史，而不是只保留一个连续天数。

### 统计

<p align="center">
  <img src="docs/screenshots/stats.svg" alt="统计界面" width="100%">
</p>

- 最上面是今天的完成率。
- 中间是最近 14 天的完成数量和完成率趋势。
- 下面是最近 16 周的打卡热力图，以及每个习惯的连续天数和 30 天完成率。
- `h` / `l` 在 7 / 14 / 30 / 90 天窗口之间切换。

### 评论

在任意界面选中一个页面、内容块或习惯，按 `C` 打开评论面板：

<p align="center">
  <img src="docs/screenshots/comments.svg" alt="评论面板" width="100%">
</p>

- `a` 添加评论（多行，`Ctrl+S` 保存）。
- `j` / `k` 选择，`e` 编辑，`r` 标记为已解决 / 取消已解决，`d` 删除。
- `Esc` 关闭。
- 评论按时间排列，显示作者和时间。未解决的评论会在对象旁边显示数量。

### 搜索和帮助

`/` 打开搜索，输入关键词会实时匹配页面标题、内容块文本和习惯名称：

<p align="center">
  <img src="docs/screenshots/search.svg" alt="搜索" width="100%">
</p>

`j` / `k` 选择，`Enter` 跳转，`Esc` 关闭。

`?` 打开快捷键帮助：

<p align="center">
  <img src="docs/screenshots/help.svg" alt="帮助面板" width="100%">
</p>

编辑和输入状态下，`Esc` 取消，`Enter` 保存（多行内容用 `Ctrl+S`）。

## 同步与协作

- 服务端记录所有变更（谁、什么时候、改了什么），客户端每 2 秒按游标拉取一遍。断网后恢复会自动追上，不需要手动操作。
- 页面和内容块带版本号。两个人同时改同一条时，后提交的一方会收到冲突，客户端自动刷新成最新版本并在底部提示，不会静默覆盖。
- 顶部显示当前成员数；别人改动页面、内容块、习惯或评论时，底部会出现提示。
- 所有人共用一个空间，没有角色和权限区分。邀请码决定了谁能加入，泄露后需要更换（`TOTION_INVITE_CODE`）。

## 架构

<p align="center">
  <img src="docs/architecture.svg" alt="架构" width="100%">
</p>

服务端是一个 axum 应用，所有数据在一个 SQLite 文件里：

- 写操作在事务内同时写实体和变更日志，避免只写一半。
- `GET /api/sync?since=N&limit=M` 返回一批变更和下一个游标，客户端循环拉到 `has_more=false`。
- 登录是 Ed25519 挑战-应答：客户端用本地私钥签一个 nonce，服务端验签后发会话 token，不传密码。
- 数据库结构由 `schema_version` 驱动迁移，启动时重复执行也不会出错。

客户端是 ratatui 界面，所有操作先写本地状态，再同步到服务端；遇到冲突用服务端返回的当前对象刷新本地。

## 开发

```bash
cargo test --workspace

# 服务端
cargo run -p totion-server -- --data-dir ./dev-data --bind 127.0.0.1:8080

# 另一个终端
cargo run -p totion-client
```

首次启动服务端后，邀请码在 `dev-data/invite_code`。

提交前跑一遍：

```bash
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Windows 上没有 MSVC 工具链时，可以用 GNU 工具链编译：

```bash
export PATH="/c/Users/hasee/scoop/apps/mingw/current/bin:$PATH"
cargo +stable-x86_64-pc-windows-gnu run -p totion-client
```

目录结构：

```
crates/totion-core/     共享类型、协议、统计与日期函数
crates/totion-server/   axum 服务、SQLite、迁移、变更日志
crates/totion-client/   ratatui TUI
docs/                   README 截图与架构图
deploy/                 systemd 单元模板
.github/workflows/      CI 与 Release
scripts/                截图生成脚本
```

## 部署

推荐用 systemd 管理服务。仓库里的 `deploy/totion.service` 是模板，把 `__DEPLOY_USER__` 和 `__DEPLOY_HOME__` 替换成实际用户和目录：

```ini
[Service]
User=admin
Group=admin
WorkingDirectory=/home/admin/totion
EnvironmentFile=-/home/admin/totion/totion.env
ExecStart=/home/admin/totion/totion-server
Restart=on-failure
```

`totion.env` 里至少要有：

```bash
TOTION_BIND=0.0.0.0:8080
TOTION_DATA_DIR=/home/admin/totion/data
TOTION_INVITE_CODE=你的邀请码
```

推送到 `main` 会跑 CI；打 `v*` tag 会构建三个平台的客户端、发布 Release，并把服务端部署到配置好的机器：

1. 上传新二进制为 `totion-server.new`
2. 停服务
3. 备份 `data/totion.db*`
4. 替换二进制、安装 systemd 单元
5. 启动并检查 `/api/health`

部署需要的 GitHub Secrets：

| Secret | 内容 |
|--------|------|
| `DEPLOY_HOST` | 服务器地址 |
| `DEPLOY_USER` | SSH 用户 |
| `DEPLOY_SSH_KEY` | 部署私钥 |
| `TOTION_INVITE_CODE` | 可选，固定邀请码 |

服务器上的文件：

```
~/totion/
  totion-server
  totion.env
  data/totion.db
  data/totion.db.bak.<时间戳>   每次部署自动备份
  data/invite_code
```

## 常见问题

**连不上服务器**

先在服务器上 `curl http://127.0.0.1:8080/api/health`。本机通、外面不通，通常是云厂商安全组没放行端口。客户端报“服务器版本过旧”说明服务端小于 v0.2，需要升级。

**邀请码在哪里**

服务端 `data/invite_code`，或者部署时设置的 `TOTION_INVITE_CODE`。已经在空间里的成员也可以在登录后看到服务器地址和成员数。

**数据存在哪，怎么备份**

全部在 `data/totion.db`。停服务后复制这个文件即可；`data/totion.db.bak.*` 是每次部署自动生成的备份。

**能不能公网使用**

可以，但当前默认是 HTTP，token 和内容会明文传输。建议在前面加 Caddy 或 Nginx 配一个域名和自动证书，客户端把地址改成 `https://` 即可，代码不用改。

**多人同时改同一个内容块会怎样**

后提交的一方会收到冲突，客户端刷新成最新内容并在底部提示。目前是块级最后写入，不是逐字合并；多人同时编辑同一段文字时，建议各自改不同块。

**适合多少人用**

当前按小规模设计，小团队（2-5 人）用最舒服。成员更多也能用，但没有角色、权限和成员管理功能。

## Roadmap

- [x] v0.1：页面 / 内容块、Ed25519 登录、增量同步、三平台 Release
- [x] v0.2：每日日记页、独立习惯与打卡历史、日历时间、统计图表、评论、搜索
- [ ] 一键 HTTPS 部署（Caddy 自动证书）
- [ ] 提醒与桌面通知
- [ ] Markdown / CSV 导出
- [ ] 成员管理与只读成员
- [ ] 移动端只读 Web 视图

## 贡献

欢迎 issue 和 PR。提交前确保 `cargo fmt --all`、`cargo clippy --workspace --all-targets -- -D warnings`、`cargo test --workspace` 都通过。细节见 [CONTRIBUTING.md](CONTRIBUTING.md)。

安全问题请不要公开提交 issue，见 [SECURITY.md](SECURITY.md)。

## License

[MIT](LICENSE)
