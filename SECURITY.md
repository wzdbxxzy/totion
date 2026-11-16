# 安全问题

请不要通过公开 issue 报告安全漏洞。

请私信联系仓库维护者（GitHub: [@wzdbxxzy](https://github.com/wzdbxxzy)），或通过 GitHub Security Advisories 私下提交。

报告中请包含：

- 受影响的版本
- 复现步骤或 PoC
- 影响范围（数据泄露 / 越权 / 拒绝服务等）

## 已知边界

- 默认部署使用 HTTP，传输层没有加密；公网部署应自行加 TLS。
- 邀请码是加入空间的唯一凭据，泄露后请更换并清理会话。
- 单实例 SQLite + 进程内锁，不支持多实例同时写入同一数据库。
