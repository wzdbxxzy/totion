#!/usr/bin/env python3
"""生成 README 用的 SVG 素材（banner、截图、架构图）。

不依赖第三方库；输出到 docs/。
"""

from __future__ import annotations

import html
import os

BG = "#0d1117"
PANEL = "#161b22"
PANEL2 = "#21262d"
BORDER = "#30363d"
TEXT = "#c9d1d9"
MUTED = "#8b949e"
CYAN = "#39d0d8"
GREEN = "#3fb950"
YELLOW = "#d29922"
MAGENTA = "#bc8cff"
RED = "#f85149"
BLUE = "#58a6ff"

FONT = "'JetBrains Mono','Fira Code','SFMono-Regular',Consolas,'Liberation Mono',monospace"


def esc(value: str) -> str:
    return html.escape(value, quote=True)


def text(x, y, value, color=TEXT, size=16, bold=False, anchor="start"):
    weight = ' font-weight="600"' if bold else ""
    return (
        f'<text x="{x}" y="{y}"{weight} fill="{color}" font-family="{FONT}" '
        f'font-size="{size}" text-anchor="{anchor}">{esc(value)}</text>'
    )


def rect(x, y, w, h, fill, stroke=None, rx=0, opacity=None):
    stroke_attr = f' stroke="{stroke}"' if stroke else ""
    opacity_attr = f' opacity="{opacity}"' if opacity is not None else ""
    return (
        f'<rect x="{x}" y="{y}" width="{w}" height="{h}" rx="{rx}" '
        f'fill="{fill}"{stroke_attr}{opacity_attr}/>'
    )


def svg(width, height, body, defs=""):
    return (
        f'<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{height}" '
        f'viewBox="0 0 {width} {height}">\n'
        f"<defs>{defs}</defs>\n"
        f"{body}\n</svg>\n"
    )


def window(width, height, title):
    clip_id = "win-clip"
    defs = f'<clipPath id="{clip_id}"><rect x="1" y="1" width="{width - 2}" height="{height - 2}" rx="14"/></clipPath>'
    body = [
        f'<g clip-path="url(#{clip_id})">',
        rect(1, 1, width - 2, height - 2, BG),
        rect(1, 1, width - 2, 40, PANEL),
        "</g>",
        rect(1, 1, width - 2, height - 2, "none", BORDER, 14),
        '<circle cx="26" cy="21" r="6" fill="#ff5f56"/>',
        '<circle cx="46" cy="21" r="6" fill="#ffbd2e"/>',
        '<circle cx="66" cy="21" r="6" fill="#27c93f"/>',
        text(width / 2, 26, title, MUTED, 13, anchor="middle"),
    ]
    return defs, "\n".join(body)


def panel(x, y, w, h, title):
    return "\n".join(
        [
            rect(x, y, w, h, PANEL, BORDER, 8),
            text(x + 14, y + 24, title, CYAN, 14, bold=True),
        ]
    )


def banner():
    width, height = 1280, 420
    defs = """
    <linearGradient id="bg" x1="0" y1="0" x2="1" y2="1">
      <stop offset="0" stop-color="#0d1117"/>
      <stop offset="1" stop-color="#161b22"/>
    </linearGradient>
    <radialGradient id="glow" cx="0.5" cy="0" r="0.8">
      <stop offset="0" stop-color="#39d0d8" stop-opacity="0.20"/>
      <stop offset="1" stop-color="#39d0d8" stop-opacity="0"/>
    </radialGradient>
    """
    body = [
        rect(0, 0, width, height, "url(#bg)"),
        rect(0, 0, width, height, "url(#glow)"),
    ]
    # 装饰网格
    for i in range(0, width, 40):
        body.append(
            f'<line x1="{i}" y1="0" x2="{i}" y2="{height}" stroke="#21262d" stroke-width="1" opacity="0.35"/>'
        )
    for i in range(0, height, 40):
        body.append(
            f'<line x1="0" y1="{i}" x2="{width}" y2="{i}" stroke="#21262d" stroke-width="1" opacity="0.35"/>'
        )

    body.append(text(80, 150, "totion", "#ffffff", 84, bold=True))
    body.append(
        text(84, 198, "A shared space in your terminal.", CYAN, 24, bold=True)
    )
    body.append(
        text(
            84,
            232,
            "Plan · Habits · Todos · Calendar · Journal — synced for two.",
            MUTED,
            18,
        )
    )

    tags = ["Rust", "ratatui", "axum", "SQLite", "Ed25519", "Self-hosted"]
    x = 84
    for tag in tags:
        width_tag = 22 + len(tag) * 9
        body.append(rect(x, 270, width_tag, 30, PANEL2, BORDER, 15))
        body.append(text(x + 12, 290, tag, TEXT, 13))
        x += width_tag + 10

    # 右侧终端卡片
    bx, by, bw, bh = 800, 92, 400, 236
    body.append(rect(bx, by, bw, bh, BG, BORDER, 12))
    body.append(rect(bx, by, bw, 32, PANEL, None, 12))
    body.append('<circle cx="818" cy="108" r="5" fill="#ff5f56"/>')
    body.append('<circle cx="834" cy="108" r="5" fill="#ffbd2e"/>')
    body.append('<circle cx="850" cy="108" r="5" fill="#27c93f"/>')
    body.append(text(bx + bw / 2, 113, "totion · today", MUTED, 11, anchor="middle"))

    lines = [
        ("2026-11-16 周一", MUTED),
        ("4/7 done  ████████░░░░  57%", GREEN),
        ("", TEXT),
        ("09:00  [x] 晨跑 5km", TEXT),
        ("10:00  [ ] 写周报", TEXT),
        ("14:30  [ ] 评审设计", TEXT),
        ("", TEXT),
        ("Habits  🔥 12  ·  2/3", YELLOW),
    ]
    y = by + 58
    for line, color in lines:
        body.append(text(bx + 20, y, line, color, 13))
        y += 20

    return svg(width, height, "\n".join(body), defs)


def today_screenshot():
    width, height = 1280, 760
    defs, body = window(width, height, "totion — 今日")

    # header 条
    body += "\n" + rect(24, 58, width - 48, 32, CYAN, None, 6)
    body += "\n" + text(
        40,
        79,
        "totion · 我的共享空间 · Guan · 2 位成员 · 今日 [1/5]",
        "#0d1117",
        15,
        bold=True,
    )

    # 日期与完成率
    body += "\n" + text(40, 124, "2026-11-16 周一", TEXT, 18, bold=True)
    body += "\n" + rect(210, 111, 260, 14, PANEL2, BORDER, 7)
    body += "\n" + rect(210, 111, 148, 14, GREEN, None, 7)
    body += "\n" + text(486, 124, "4/7   57%", MUTED, 15)

    # 今日事项
    body += "\n" + panel(24, 146, 700, 250, " 今日事项 ")
    items = [
        ("09:00", "[x]", "晨跑 5km", GREEN, TEXT),
        ("10:00", "[ ]", "写周报", MUTED, TEXT),
        ("14:30", "[ ]", "评审新设计", MUTED, TEXT),
        ("18:00", "[ ]", "买菜", MUTED, TEXT),
    ]
    y = 194
    for time, mark, title, mark_color, title_color in items:
        body += "\n" + text(48, y, time, CYAN, 15)
        body += "\n" + text(112, y, mark, mark_color, 15)
        body += "\n" + text(156, y, title, title_color, 15)
        y += 40

    # 今日习惯
    body += "\n" + panel(744, 146, 512, 250, " 今日习惯 ")
    habits = [
        ("[x]", "📖 阅读 30 分钟", "🔥 12", GREEN),
        ("[ ]", "🏃 跑步", "本周 2/3", YELLOW),
        ("[ ]", "🧘 冥想", "本周 1/4", MUTED),
    ]
    y = 194
    for mark, title, extra, color in habits:
        body += "\n" + text(768, y, mark, GREEN if mark == "[x]" else MUTED, 15)
        body += "\n" + text(812, y, title, TEXT, 15)
        body += "\n" + text(1060, y, extra, color, 14)
        y += 40

    # 日记
    body += "\n" + panel(24, 414, 1232, 240, " 日记：总结 / 感悟 / 经验 ")
    diary = [
        ("总结", "完成了同步协议重构，迁移测试全部通过。"),
        ("感悟", "小步提交 + 自动部署，比一次性大改轻松很多。"),
        ("经验", "涉及 schema 变更时，先在备份库上跑迁移。"),
    ]
    y = 462
    for label, value in diary:
        body += "\n" + text(48, y, f"{label}:", CYAN, 15, bold=True)
        body += "\n" + text(116, y, value, TEXT, 15)
        y += 46

    body += "\n" + text(40, 686, "已连接 · 2 秒前同步", MUTED, 13)
    body += "\n" + text(
        40,
        716,
        "1-5 视图 · j/k 移动 · Space 打卡 · Enter 编辑 · C 评论 · / 搜索 · ? 帮助",
        MUTED,
        13,
    )
    return svg(width, height, body, defs)


def habits_screenshot():
    width, height = 1280, 720
    defs, body = window(width, height, "totion — 习惯")

    body += "\n" + rect(24, 58, width - 48, 32, CYAN, None, 6)
    body += "\n" + text(40, 79, "totion · 我的共享空间 · Guan · 2 位成员 · 习惯 [4/5]", "#0d1117", 15, bold=True)

    body += "\n" + panel(24, 110, 430, 560, " 习惯 ")
    habits = [
        ("[x]", "📖 阅读 30 分钟", "🔥12  本周 5 次", True),
        ("[ ]", "🏃 跑步", "🔥3  本周 2/3", False),
        ("[ ]", "🧘 冥想", "🔥1  本周 1/4", False),
        ("[ ]", "💧 喝 2L 水", "🔥7  本周 4 次", False),
    ]
    y = 158
    for mark, title, extra, selected in habits:
        if selected:
            body += "\n" + rect(36, y - 26, 406, 42, PANEL2, None, 6)
        body += "\n" + text(50, y, mark, GREEN if mark == "[x]" else MUTED, 15)
        body += "\n" + text(94, y, title, TEXT, 15)
        body += "\n" + text(300, y, extra, YELLOW, 12)
        y += 58

    body += "\n" + panel(474, 110, 782, 560, " 详情 · 阅读 30 分钟 ")
    body += "\n" + text(500, 160, "📖 阅读 30 分钟", TEXT, 22, bold=True)
    body += "\n" + text(500, 196, "频率: 每天", MUTED, 14)
    body += "\n" + text(640, 196, "连续: 12 天", MUTED, 14)
    body += "\n" + text(780, 196, "最长: 21 天", MUTED, 14)

    body += "\n" + text(500, 236, "30 天完成率", MUTED, 14)
    body += "\n" + rect(620, 224, 340, 16, PANEL2, BORDER, 8)
    body += "\n" + rect(620, 224, 282, 16, GREEN, None, 8)
    body += "\n" + text(976, 237, "83%", GREEN, 14, bold=True)

    body += "\n" + text(500, 286, "最近 12 周", MUTED, 14)
    # 热力图：12 列 x 7 行
    start_x, start_y, cell, gap = 500, 310, 20, 8
    for week in range(12):
        for day in range(7):
            x = start_x + week * (cell + gap)
            y = start_y + day * (cell + gap)
            on = (week * 7 + day * 5 + 3) % 4 != 0
            color = GREEN if on else "#1c2128"
            body += "\n" + rect(x, y, cell, cell, color, "#2d333b", 3)

    body += "\n" + text(500, 510, "最近打卡: 11-16, 11-15, 11-14, 11-12", MUTED, 13)
    body += "\n" + text(500, 546, "备注: 每天睡前 30 分钟，非虚构类优先。", MUTED, 13)
    body += "\n" + text(500, 610, "Space 今天打卡 · u 撤销 · e 改名称 · C 评论", MUTED, 13)
    return svg(width, height, body, defs)


def calendar_screenshot():
    width, height = 1280, 720
    defs, body = window(width, height, "totion — 日历")

    body += "\n" + rect(24, 58, width - 48, 32, CYAN, None, 6)
    body += "\n" + text(40, 79, "totion · 我的共享空间 · Guan · 2 位成员 · 日历 [3/5]", "#0d1117", 15, bold=True)

    body += "\n" + panel(24, 110, 700, 560, " 2026 年 11 月（←/→ 选日 · [ ] 换月） ")

    weekdays = ["一", "二", "三", "四", "五", "六", "日"]
    for index, label in enumerate(weekdays):
        body += "\n" + text(58 + index * 94, 160, label, MUTED, 14, bold=True)

    # 2026-11-01 是周日，周一开头偏移 6 天
    offset = 6
    days = 30
    events = {3, 5, 8, 12, 16, 18, 21, 24, 27}
    for index in range(42):
        row = index // 7
        column = index % 7
        day = index - offset + 1
        x = 46 + column * 94
        y = 180 + row * 76
        if day < 1 or day > days:
            continue
        body += "\n" + rect(x, y, 84, 62, PANEL2, BORDER, 6)
        if day == 16:
            body += "\n" + rect(x, y, 84, 62, CYAN, None, 6)
            body += "\n" + text(x + 10, y + 26, str(day), "#0d1117", 16, bold=True)
        else:
            body += "\n" + text(x + 10, y + 26, str(day), TEXT, 16)
        if day in events:
            body += "\n" + text(x + 54, y + 26, "•", GREEN, 18, bold=True)
        if day == 16:
            body += "\n" + text(x + 10, y + 48, "4 项", "#0d1117", 12)

    body += "\n" + panel(744, 110, 512, 560, " 11 月 16 日 议程 ")
    agenda = [
        ("09:00", "[x]", "晨会", GREEN),
        ("10:30", "[ ]", "写周报", MUTED),
        ("14:30", "[ ]", "设计评审", MUTED),
        ("18:00", "[ ]", "买菜", MUTED),
    ]
    y = 170
    for time, mark, title, color in agenda:
        body += "\n" + text(768, y, time, CYAN, 15)
        body += "\n" + text(830, y, mark, color, 15)
        body += "\n" + text(874, y, title, TEXT, 15)
        y += 52

    body += "\n" + text(768, 420, "t 新建待办   c 新建日程", MUTED, 13)
    body += "\n" + text(768, 452, "Space 勾选   Enter 编辑   C 评论", MUTED, 13)
    return svg(width, height, body, defs)


def stats_screenshot():
    width, height = 1280, 760
    defs, body = window(width, height, "totion — 统计")

    body += "\n" + rect(24, 58, width - 48, 32, CYAN, None, 6)
    body += "\n" + text(40, 79, "totion · 我的共享空间 · Guan · 2 位成员 · 统计 [5/5]", "#0d1117", 15, bold=True)

    body += "\n" + panel(24, 110, 1232, 78, " 今日完成率 ")
    body += "\n" + rect(48, 152, 1184, 18, PANEL2, BORDER, 9)
    body += "\n" + rect(48, 152, 676, 18, GREEN, None, 9)
    body += "\n" + text(1140, 167, "4/7  57%", GREEN, 14, bold=True)

    body += "\n" + panel(24, 204, 600, 150, " 最近 14 天完成数 ")
    values = [2, 4, 3, 5, 1, 6, 4, 3, 5, 2, 4, 6, 3, 4]
    base_y = 330
    for index, value in enumerate(values):
        x = 52 + index * 40
        height_bar = value * 12
        body += "\n" + rect(x, base_y - height_bar, 24, height_bar, CYAN, None, 3)

    body += "\n" + panel(656, 204, 600, 150, " 最近 14 天完成率（%） ")
    points = []
    rates = [30, 55, 48, 70, 20, 82, 64, 50, 74, 38, 60, 88, 52, 66]
    for index, rate in enumerate(rates):
        x = 686 + index * 39
        y = 330 - rate * 1.15
        points.append(f"{x},{y:.1f}")
    body += "\n" + f'<polyline points="{" ".join(points)}" fill="none" stroke="{GREEN}" stroke-width="3"/>'

    body += "\n" + panel(24, 370, 1232, 230, " 最近 16 周打卡热力图 ")
    start_x, start_y, cell, gap = 60, 406, 20, 6
    for week in range(16):
        for day in range(7):
            x = start_x + week * (cell + gap)
            y = start_y + day * (cell + gap)
            on = (week * 7 + day * 3) % 5 != 0
            body += "\n" + rect(x, y, cell, cell, GREEN if on else "#1c2128", "#2d333b", 4)

    body += "\n" + panel(24, 612, 1232, 110, " 习惯统计 ")
    rows = [
        ("📖 阅读 30 分钟", "连续 12 天", "30 天 83%", "本周 5 次"),
        ("🏃 跑步", "连续 3 天", "30 天 61%", "本周 2/3"),
        ("🧘 冥想", "连续 1 天", "30 天 44%", "本周 1/4"),
    ]
    y = 656
    for icon_title, streak, rate, weekly in rows:
        body += "\n" + text(48, y, icon_title, TEXT, 15)
        body += "\n" + text(360, y, streak, MUTED, 14)
        body += "\n" + text(600, y, rate, GREEN, 14)
        body += "\n" + text(840, y, weekly, YELLOW, 14)
        y += 28

    return svg(width, height, body, defs)


def architecture_screenshot():
    width, height = 1200, 620
    defs = """
    <marker id="arrow" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="7" markerHeight="7" orient="auto-start-reverse">
      <path d="M 0 0 L 10 5 L 0 10 z" fill="#8b949e"/>
    </marker>
    """
    body = [rect(0, 0, width, height, BG)]

    body.append(rect(40, 90, 300, 220, PANEL, BORDER, 12))
    body.append(text(60, 126, "TUI Client", TEXT, 20, bold=True))
    body.append(text(60, 154, "Rust · ratatui", MUTED, 14))
    body.append(text(60, 190, "Windows", TEXT, 14))
    body.append(text(60, 216, "macOS (arm64 / x64)", TEXT, 14))
    body.append(text(60, 242, "Linux", TEXT, 14))
    body.append(text(60, 282, "本地缓存 + 乐观锁", MUTED, 13))

    body.append(rect(480, 90, 280, 220, PANEL, BORDER, 12))
    body.append(text(500, 126, "Sync", TEXT, 20, bold=True))
    body.append(text(500, 158, "HTTP / JSON", MUTED, 14))
    body.append(text(500, 188, "Ed25519 挑战登录", TEXT, 14))
    body.append(text(500, 214, "GET /api/sync?since=N", TEXT, 13))
    body.append(text(500, 240, "每 2 秒增量拉取", TEXT, 14))
    body.append(text(500, 280, "change log + cursor", MUTED, 13))

    body.append(rect(880, 90, 280, 220, PANEL, BORDER, 12))
    body.append(text(900, 126, "Server", TEXT, 20, bold=True))
    body.append(text(900, 156, "Rust · axum", MUTED, 14))
    body.append(text(900, 188, "SQLite (WAL)", TEXT, 14))
    body.append(text(900, 214, "pages / blocks", TEXT, 13))
    body.append(text(900, 240, "habits / checkins / comments", TEXT, 12))
    body.append(text(900, 280, "systemd · GaryServer", MUTED, 13))

    body.append(
        f'<line x1="340" y1="200" x2="472" y2="200" stroke="{MUTED}" stroke-width="2.5" marker-end="url(#arrow)"/>'
    )
    body.append(
        f'<line x1="472" y1="222" x2="340" y2="222" stroke="{MUTED}" stroke-width="2.5" marker-end="url(#arrow)"/>'
    )
    body.append(
        f'<line x1="760" y1="200" x2="872" y2="200" stroke="{MUTED}" stroke-width="2.5" marker-end="url(#arrow)"/>'
    )
    body.append(
        f'<line x1="872" y1="222" x2="760" y2="222" stroke="{MUTED}" stroke-width="2.5" marker-end="url(#arrow)"/>'
    )

    body.append(rect(40, 370, 1120, 190, PANEL, BORDER, 12))
    body.append(text(64, 406, "CI / CD", TEXT, 20, bold=True))
    steps = [
        ("push main", "fmt · clippy · test"),
        ("tag v*", "3 平台构建 + Release"),
        ("deploy", "scp · 备份 · systemctl"),
        ("health", "/api/health 校验"),
    ]
    for index, (title, detail) in enumerate(steps):
        x = 64 + index * 278
        body.append(rect(x, 430, 236, 96, PANEL2, BORDER, 10))
        body.append(text(x + 18, 466, title, CYAN, 16, bold=True))
        body.append(text(x + 18, 494, detail, TEXT, 13))
        if index < 3:
            body.append(
                f'<line x1="{x + 236}" y1="478" x2="{x + 274}" y2="478" stroke="{MUTED}" stroke-width="2.5" marker-end="url(#arrow)"/>'
            )

    body.append(text(64, 586, "SQLite 单实例 + 变更日志；客户端离线时从游标继续追赶。", MUTED, 13))
    return svg(width, height, "\n".join(body), defs)


def main():
    out_dir = os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))), "docs")
    screenshots = os.path.join(out_dir, "screenshots")
    os.makedirs(screenshots, exist_ok=True)

    files = {
        os.path.join(out_dir, "banner.svg"): banner(),
        os.path.join(out_dir, "architecture.svg"): architecture_screenshot(),
        os.path.join(screenshots, "today.svg"): today_screenshot(),
        os.path.join(screenshots, "habits.svg"): habits_screenshot(),
        os.path.join(screenshots, "calendar.svg"): calendar_screenshot(),
        os.path.join(screenshots, "stats.svg"): stats_screenshot(),
    }
    for path, content in files.items():
        with open(path, "w", encoding="utf-8") as handle:
            handle.write(content)
        print(f"wrote {path} ({len(content)} bytes)")


if __name__ == "__main__":
    main()
