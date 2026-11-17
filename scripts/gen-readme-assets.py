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
            "Plan · Habits · Todos · Calendar · Journal — synced for a small group.",
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
        "totion · 我的共享空间 · Guan · 3 位成员 · 今日 [1/6]",
        "#0d1117",
        15,
        bold=True,
    )

    # 日期与完成率
    body += "\n" + text(40, 124, "2026-11-16 今天", TEXT, 18, bold=True)
    body += "\n" + rect(270, 111, 220, 14, PANEL2, BORDER, 7)
    body += "\n" + rect(270, 111, 125, 14, GREEN, None, 7)
    body += "\n" + text(506, 124, "4/7   57%", MUTED, 15)
    body += "\n" + text(650, 124, "h/l 或 [ ] 切换日期 · w 近一周", MUTED, 13)

    # 今日待办
    body += "\n" + panel(24, 146, 700, 250, " 今日待办 [Tab] ")
    items = [
        ("09:00", "✓", "晨跑 5km", GREEN, TEXT),
        ("10:00", "○", "写周报", MUTED, TEXT),
        ("14:30", "○", "评审新设计", MUTED, TEXT),
        ("18:00", "○", "买菜", MUTED, TEXT),
    ]
    y = 194
    for time, mark, title, mark_color, title_color in items:
        body += "\n" + text(48, y, time, CYAN, 15)
        body += "\n" + text(112, y, mark, mark_color, 15)
        body += "\n" + text(156, y, title, title_color, 15)
        y += 40

    # 今日习惯
    body += "\n" + panel(744, 146, 512, 250, " 今日习惯 [Tab] ")
    habits = [
        ("✓", "📖 阅读 30 分钟", "🔥 12 · 本周 5 次", GREEN),
        ("○", "🏃 跑步", "本周 2/3", YELLOW),
        ("○", "🧘 冥想", "本周 1/4", MUTED),
    ]
    y = 194
    for mark, title, extra, color in habits:
        body += "\n" + text(768, y, mark, GREEN if mark == "✓" else MUTED, 15)
        body += "\n" + text(812, y, title, TEXT, 15)
        body += "\n" + text(1010, y, extra, color, 13)
        y += 40

    # 总结（单条文本框）+ 评论预览
    body += "\n" + panel(24, 414, 820, 240, " 总结 [Tab] ")
    body += "\n" + text(48, 466, "21:30 · Guan", CYAN, 14)
    body += "\n" + text(48, 500, "周报写完了，明天给同事过一遍。迁移测试也过了。", TEXT, 15)

    body += "\n" + panel(868, 414, 388, 240, " 评论 ")
    body += "\n" + text(892, 466, "队友：迁移测试过了吗？", TEXT, 14)
    body += "\n" + text(892, 500, "Guan：过了，数据库也备份了。", MUTED, 14)

    body += "\n" + text(40, 686, "已连接 · 2 秒前同步", MUTED, 13)
    body += "\n" + text(
        40,
        716,
        "j/k 移动 · Space 勾选 · Enter 编辑 · d 删除 · a 总结 · t 新建 · h/l 切天 · w 周 · Tab 面板",
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
    width, height = 1280, 760
    defs, body = window(width, height, "totion — 日历")

    body += "\n" + rect(24, 58, width - 48, 32, CYAN, None, 6)
    body += "\n" + text(
        40,
        79,
        "totion · 我的共享空间 · Guan · 3 位成员 · 日历 [3/6]",
        "#0d1117",
        15,
        bold=True,
    )

    body += "\n" + text(
        640,
        122,
        "🍁   2026 年 11 月   🍁",
        CYAN,
        24,
        bold=True,
        anchor="middle",
    )
    body += "\n" + text(
        640,
        148,
        "←/→ 选日 · [ ] 换月 · v 或 Shift+右键框选   已选 11-16 ~ 11-20",
        MUTED,
        13,
        anchor="middle",
    )

    weekdays = ["一", "二", "三", "四", "五", "六", "日"]
    for index, label in enumerate(weekdays):
        body += "\n" + text(40 + index * 174, 174, label, MUTED, 14, bold=True)

    # 2026-11-01 是周日，周一开头偏移 6 天
    offset = 6
    days = 30
    selected = 16
    today = 17
    range_start = 16
    range_end = 20
    events = {
        3: [("✓", "晨跑", GREEN)],
        5: [("○", "写周报", MUTED)],
        8: [("○", "复盘会", MUTED)],
        12: [("○", "评审", MUTED)],
        16: [("✓", "晨会", GREEN), ("○", "写周报", MUTED), ("○", "评审", MUTED)],
        17: [("○", "买菜", MUTED)],
        18: [("○", "买菜", MUTED)],
        21: [("○", "周复盘", MUTED)],
        24: [("○", "设计评审", MUTED)],
        27: [("✓", "跑步", GREEN)],
    }
    for index in range(42):
        row = index // 7
        column = index % 7
        day = index - offset + 1
        if day < 1 or day > days:
            continue
        x = 24 + column * 174
        y = 184 + row * 88
        in_range = range_start <= day <= range_end
        if day == selected:
            fill, border = "#0e4b52", CYAN
        elif day == today:
            fill, border = "#103c24", GREEN
        elif in_range:
            fill, border = "#222b33", "#4b6a7a"
        elif day < today:
            fill, border = "#12161b", "#2a3038"
        else:
            fill, border = PANEL2, BORDER
        body += "\n" + rect(x, y, 166, 82, fill, border, 6)
        body += "\n" + text(
            x + 10,
            y + 24,
            str(day),
            "#ffffff" if day == selected else (GREEN if day == today else (MUTED if day < today else TEXT)),
            16,
            bold=True,
        )
        for line_index, (mark, title, color) in enumerate(events.get(day, [])[:3]):
            body += "\n" + text(x + 10, y + 46 + line_index * 18, mark, color, 12, bold=True)
            body += "\n" + text(x + 28, y + 46 + line_index * 18, title, TEXT, 12)

    body += "\n" + text(
        40,
        746,
        "11-16 · Enter 议程 · t 新建待办 · v 框选 · Esc 取消",
        MUTED,
        13,
    )
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


def setup_screenshot():
    width, height = 1280, 720
    defs, body = window(width, height, "totion — 首次连接")
    x, y, w, h = 320, 150, 640, 360
    body += "\n" + rect(x, y, w, h, PANEL, BORDER, 12)
    body += "\n" + text(x + 32, y + 52, "totion · 连接共享空间", CYAN, 22, bold=True)
    body += "\n" + text(
        x + 32, y + 88, "第一次使用需要邀请码注册，之后自动免密登录。", MUTED, 14
    )
    fields = [
        ("服务器地址", "http://your-server:8080", True),
        ("名字", "Guan", False),
        ("邀请码", "AB12-CD34", False),
    ]
    fy = y + 146
    for label, value, active in fields:
        prefix = "▶ " if active else "  "
        line = f"{prefix}{label}: {value}" + ("█" if active else "")
        body += "\n" + text(
            x + 32, fy, line, CYAN if active else TEXT, 16, bold=active
        )
        fy += 48
    body += "\n" + text(
        x + 32,
        y + 306,
        "Tab 切换字段   Enter 连接   Ctrl+R 重置密钥   Ctrl+C 退出",
        MUTED,
        13,
    )
    return svg(width, height, body, defs)


def workspace_screenshot():
    width, height = 1280, 720
    defs, body = window(width, height, "totion — 工作区")

    body += "\n" + rect(24, 58, width - 48, 32, CYAN, None, 6)
    body += "\n" + text(
        40, 79, "totion · 我的共享空间 · Guan · 3 位成员 · 工作区 [2/5]", "#0d1117", 15, bold=True
    )

    body += "\n" + panel(24, 110, 380, 560, " 页面 ")
    pages = [
        ("🎯 计划", 0),
        ("▶ 🎯 2026 计划", 0),
        ("  📄 周会记录", 0),
        ("  📄 数据库迁移笔记", 0),
        ("📅 日历", 0),
        ("  📅 十一月", 0),
        ("✅ 待办", 0),
        ("  ✅ 本周", 0),
    ]
    y = 156
    for label, _ in pages:
        if label.strip() == "2026 计划":
            body += "\n" + rect(34, y - 22, 360, 34, PANEL2, None, 6)
        body += "\n" + text(46, y, label, TEXT, 15)
        y += 44

    body += "\n" + panel(424, 110, 832, 348, " 2026 计划 ")
    rows = [
        ("# 2026 计划", TEXT, True),
        ("", TEXT, False),
        ("[ ] 完成 v0.2 发布   [2]", TEXT, False),
        ("[x] 写迁移测试", GREEN, False),
        ("· 明天把更新同步给队友", TEXT, False),
        ("○ 复盘会", TEXT, False),
        ("09:00  [ ] 晨跑 5km", TEXT, False),
        ("· 备注：迁移前先备份数据库", TEXT, False),
    ]
    y = 154
    for value, color, bold in rows:
        body += "\n" + text(448, y, value, color, 15, bold=bold)
        y += 38

    # 右下常驻「全部总结」
    body += "\n" + panel(424, 476, 832, 194, " 全部总结 · J 展开 ")
    journals = [
        ("2026-11-16", "[待办]", "买菜", "菜市场的番茄便宜，顺手记一下。"),
        ("2026-11-16", "[习惯]", "阅读 30 分钟", "读到第四章，讲增量同步的取舍。"),
        ("2026-11-15", "[习惯]", "跑步", "慢跑 4km，膝盖状态还行。"),
        ("2026-11-15", "[待办]", "评审新设计", "把配色和日历格子再对齐一版。"),
    ]
    y = 512
    for date, tag, label, preview in journals:
        body += "\n" + text(448, y, date, CYAN, 13)
        body += "\n" + text(556, y, tag, YELLOW, 13)
        body += "\n" + text(624, y, label, TEXT, 13, bold=True)
        body += "\n" + text(796, y, preview, MUTED, 13)
        y += 36

    body += "\n" + text(
        40, 700, "Tab 页面/内容 · j/k 移动 · Enter 编辑 · n 文本 · t 待办 · N 新页面 · a 每日页 · d 删除 · J 全部总结 · C 评论", MUTED, 13
    )
    return svg(width, height, body, defs)


def comments_screenshot():
    width, height = 1280, 720
    defs, body = window(width, height, "totion — 评论")

    body += "\n" + rect(24, 58, width - 48, 32, CYAN, None, 6)
    body += "\n" + text(
        40, 79, "totion · 我的共享空间 · Guan · 3 位成员 · 工作区 [2/5]", "#0d1117", 15, bold=True
    )

    # 背景主界面（简化）
    body += "\n" + panel(24, 110, 620, 560, " 2026 计划 ")
    background_rows = [
        ("# 2026 计划", TEXT),
        ("[ ] 完成 v0.2 发布   [2]", TEXT),
        ("[x] 写迁移测试", GREEN),
        ("· 明天把更新同步给队友", TEXT),
    ]
    y = 158
    for value, color in background_rows:
        body += "\n" + text(48, y, value, color, 15)
        y += 44

    # 评论面板
    px = 700
    body += "\n" + rect(px, 40, width - px - 1, height - 41, PANEL, BORDER, 0)
    body += "\n" + text(
        px + 24, 76, "评论 · 内容块「完成 v0.2 发布」", CYAN, 16, bold=True
    )

    comments = [
        ("[ ]", "Guan", "11-16 14:32", "迁移测试已经过了吗？", False),
        ("[✓]", "队友", "11-16 15:10", "过了，数据库也备份了。", True),
        ("[ ]", "Guan", "11-16 15:12", "那今晚发 v0.2.0。", False),
    ]
    y = 126
    for mark, author, time, content, resolved in comments:
        if resolved:
            body += "\n" + rect(px + 16, y - 24, width - px - 40, 84, PANEL2, None, 8)
        body += "\n" + text(px + 28, y, mark, GREEN if resolved else MUTED, 15)
        body += "\n" + text(px + 72, y, author, TEXT, 15, bold=True)
        body += "\n" + text(px + 180, y, time, MUTED, 12)
        body += "\n" + text(px + 28, y + 32, content, TEXT, 14)
        y += 96

    body += "\n" + text(
        px + 24, 640, "a 添加 · e 编辑 · r 解决/取消 · d 删除 · Esc 关闭", MUTED, 13
    )
    return svg(width, height, body, defs)


def search_screenshot():
    width, height = 1280, 720
    defs, body = window(width, height, "totion — 搜索")

    x, y, w, h = 200, 120, 880, 480
    body += "\n" + rect(x, y, w, h, PANEL, BORDER, 12)
    body += "\n" + text(x + 28, y + 44, "搜索: 迁移█", CYAN, 20, bold=True)
    results = [
        ("[页面]", "2026 计划", True),
        ("[内容块]", "写迁移测试", False),
        ("[内容块]", "数据库迁移笔记", False),
        ("[习惯]", "阅读 30 分钟", False),
    ]
    ry = y + 104
    for kind, value, selected in results:
        if selected:
            body += "\n" + rect(x + 18, ry - 26, w - 36, 40, PANEL2, None, 6)
        body += "\n" + text(x + 36, ry, kind, MUTED, 14)
        body += "\n" + text(x + 150, ry, value, TEXT, 16, bold=selected)
        ry += 54

    body += "\n" + text(
        x + 28, y + h - 24, "j/k 选择 · Enter 跳转 · Esc 关闭", MUTED, 13
    )
    return svg(width, height, body, defs)


def help_screenshot():
    width, height = 1280, 720
    defs, body = window(width, height, "totion — 帮助")

    x, y, w, h = 180, 70, 920, 580
    body += "\n" + rect(x, y, w, h, PANEL, BORDER, 12)
    body += "\n" + text(x + 30, y + 44, "帮助 · v0.8.1", CYAN, 18, bold=True)

    sections = [
        ("视图", "1 今日   2 工作区   3 日历   4 习惯   5 统计   6 留言"),
        ("今日", "Tab 待办/习惯/总结；h/l 或 [ ] 切日期；w 近一周；a 写总结"),
        ("工作区", "Tab 页面/内容；右下常驻全部总结；J 全屏；n 文本；t 待办"),
        ("日历", "h/l 选日；j/k 前后一周；[ ] 换月；Enter 议程；v 框选"),
        ("批量", "选中日期范围后按 t 批量新建待办（最多 62 天）"),
        ("习惯", "n 新建；d 归档；Space 打卡；u 撤销；Ctrl+F 切换频率"),
        ("删除", "按 d 后输入 delete 并回车确认；Esc 取消"),
        ("协作", "C 评论；6 留言；m 成员视角；i 邀请码；/ 搜索；r 同步"),
        ("其他", "? 帮助；q 或 Ctrl+C 退出"),
    ]
    line_y = y + 90
    for title, detail in sections:
        body += "\n" + text(x + 30, line_y, title, TEXT, 15, bold=True)
        body += "\n" + text(x + 150, line_y, detail, MUTED, 14)
        line_y += 52

    body += "\n" + text(x + 30, y + h - 28, "按 Esc 关闭", MUTED, 13)
    return svg(width, height, body, defs)


def new_event_screenshot():
    width, height = 1280, 720
    defs, body = window(width, height, "totion — 新建待办")

    x, y, w, h = 330, 190, 620, 320
    body += "\n" + rect(x, y, w, h, PANEL, BORDER, 12)
    body += "\n" + text(x + 30, y + 48, "新建待办", CYAN, 20, bold=True)

    fields = [
        ("标题", "写迁移测试", True),
        ("日期", "2026-11-16", False),
        ("开始", "15:00", False),
        ("结束", "16:00", False),
    ]
    fy = y + 100
    for label, value, active in fields:
        prefix = "▶ " if active else "  "
        line = f"{prefix}{label}: {value}" + ("█" if active else "")
        body += "\n" + text(x + 30, fy, line, CYAN if active else TEXT, 15, bold=active)
        fy += 40

    body += "\n" + text(
        x + 30, y + h - 28, "Tab 切换字段 · Enter 提交 · Esc 取消", MUTED, 13
    )
    return svg(width, height, body, defs)


def inbox_screenshot():
    width, height = 1280, 760
    defs, body = window(width, height, "totion — 留言")

    body += "\n" + rect(24, 58, width - 48, 32, CYAN, None, 6)
    body += "\n" + text(
        40,
        79,
        "totion · 我的共享空间 · Guan · 3 位成员 · 留言 [6/6]",
        "#0d1117",
        15,
        bold=True,
    )

    body += "\n" + text(40, 116, "留言 · 2 条未解决", CYAN, 18, bold=True)

    comments = [
        ("[未解决]", "队友", "11-16 15:10", "内容块「写迁移测试」", "迁移测试已经过了吗？", True),
        ("[已解决]", "Guan", "11-16 14:32", "页面「2026 计划」", "今晚发版可以吗？", False),
        ("[未解决]", "队友", "11-16 16:02", "习惯「阅读 30 分钟」", "今天记得打卡。", False),
    ]
    y = 150
    for mark, author, time, target, content, selected in comments:
        if selected:
            body += "\n" + rect(30, y - 22, 1220, 100, "#222b33", None, 8)
        body += "\n" + text(
            44,
            y,
            mark,
            YELLOW if mark == "[未解决]" else GREEN,
            14,
            bold=True,
        )
        body += "\n" + text(170, y, author, TEXT, 15, bold=True)
        body += "\n" + text(300, y, time, MUTED, 12)
        body += "\n" + text(460, y, target, CYAN, 14)
        body += "\n" + text(60, y + 30, content, TEXT, 15)
        y += 112

    body += "\n" + text(
        40,
        730,
        "j/k 选择 · Enter 跳转 · a 回复 · r 解决/取消 · e 编辑 · d 删除",
        MUTED,
        13,
    )
    return svg(width, height, body, defs)


def invites_screenshot():
    width, height = 1280, 720
    defs, body = window(width, height, "totion — 邀请码")

    x, y, w, h = 250, 110, 780, 480
    body += "\n" + rect(x, y, w, h, PANEL, BORDER, 12)
    body += "\n" + text(x + 30, y + 46, "邀请码", CYAN, 20, bold=True)
    body += "\n" + text(x + 30, y + 72, "同一个空间的有效邀请码都能用来注册", MUTED, 13)

    invites = [
        ("▶ AB12CD34", "有效", "已用 0/1 · 不过期", "给小明 · Guan", True),
        ("  EF56GH78", "已撤销", "已用 2 次 · 2027-01-01 00:00", "临时码 · 队友", False),
        ("  JK90LM12", "有效", "已用 3 次 · 不过期", "家人 · Guan", False),
    ]
    line_y = y + 118
    for code, status, detail, note, selected in invites:
        if selected:
            body += "\n" + rect(x + 18, line_y - 26, w - 36, 82, PANEL2, None, 8)
        body += "\n" + text(x + 30, line_y, code, "#ffffff", 18, bold=True)
        body += "\n" + text(
            x + 210,
            line_y,
            status,
            GREEN if status == "有效" else MUTED,
            14,
            bold=True,
        )
        body += "\n" + text(x + 300, line_y, detail, MUTED, 13)
        body += "\n" + text(x + 38, line_y + 30, note, TEXT, 14)
        line_y += 96

    body += "\n" + text(
        x + 30,
        y + h - 26,
        "g 生成新码 · x 撤销/恢复 · j/k 选择 · Esc 关闭",
        MUTED,
        13,
    )
    return svg(width, height, body, defs)


def members_screenshot():
    width, height = 1280, 640
    defs, body = window(width, height, "totion — 成员视角")

    body += "\n" + rect(24, 58, width - 48, 32, CYAN, None, 6)
    body += "\n" + text(
        40,
        79,
        "totion · 我的共享空间 · Guan · 3 位成员 · 今日 [1/6]",
        "#0d1117",
        15,
        bold=True,
    )

    x, y, w, h = 300, 120, 680, 420
    body += "\n" + rect(x, y, w, h, PANEL, BORDER, 12)
    body += "\n" + text(x + 28, y + 40, "成员视角 · m", CYAN, 18, bold=True)
    body += "\n" + text(x + 28, y + 74, "选择一个成员，切换到对方的 1-6 界面", MUTED, 13)

    rows = [
        ("[共享] ", "共享空间（全部数据）", ""),
        ("[成员] ", "Guan", "（我）"),
        ("[成员] ", "小明", ""),
        ("[成员] ", "小红", ""),
    ]
    row_y = y + 118
    for index, (tag, name, suffix) in enumerate(rows):
        if index == 2:
            body += "\n" + rect(x + 16, row_y - 24, w - 32, 34, PANEL2, None, 6)
            mark = "▶ "
        else:
            mark = "  "
        body += "\n" + text(x + 28, row_y, mark, CYAN, 15)
        body += "\n" + text(x + 60, row_y, tag, YELLOW, 14)
        body += "\n" + text(x + 130, row_y, name + suffix, TEXT, 15, bold=(index == 2))
        row_y += 52

    body += "\n" + text(
        x + 28,
        y + h - 30,
        "他人视角只读，可以评论；Enter 切换，Esc 关闭",
        MUTED,
        13,
    )
    return svg(width, height, body, defs)


def main():
    out_dir = os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))), "docs")
    screenshots = os.path.join(out_dir, "screenshots")
    os.makedirs(screenshots, exist_ok=True)

    files = {
        os.path.join(out_dir, "banner.svg"): banner(),
        os.path.join(out_dir, "architecture.svg"): architecture_screenshot(),
        os.path.join(screenshots, "setup.svg"): setup_screenshot(),
        os.path.join(screenshots, "today.svg"): today_screenshot(),
        os.path.join(screenshots, "workspace.svg"): workspace_screenshot(),
        os.path.join(screenshots, "calendar.svg"): calendar_screenshot(),
        os.path.join(screenshots, "habits.svg"): habits_screenshot(),
        os.path.join(screenshots, "stats.svg"): stats_screenshot(),
        os.path.join(screenshots, "comments.svg"): comments_screenshot(),
        os.path.join(screenshots, "search.svg"): search_screenshot(),
        os.path.join(screenshots, "inbox.svg"): inbox_screenshot(),
        os.path.join(screenshots, "invites.svg"): invites_screenshot(),
        os.path.join(screenshots, "members.svg"): members_screenshot(),
        os.path.join(screenshots, "help.svg"): help_screenshot(),
        os.path.join(screenshots, "new-event.svg"): new_event_screenshot(),
    }
    for path, content in files.items():
        with open(path, "w", encoding="utf-8") as handle:
            handle.write(content)
        print(f"wrote {path} ({len(content)} bytes)")


if __name__ == "__main__":
    main()
