---
name: muse-creator
description: 面向 Muse 使用者的创作入口。用户想把一个想法、故事、情绪或参考作品变成可编辑的视听项目，尚未明确自己属于作曲、科普、短视频、电影或游戏方向时使用；负责澄清目标并选择最合适的 Muse 创作 skill。
---

# Muse 创作入口

先理解作品，不要先输出命令或语法。把自然语言整理成创作简报：受众、媒介、时长、画幅、情绪弧、声音重点、视觉重点、发布平台和可用素材。

## 选择路径

- 有旋律、和声、配器或乐谱目标：转交 `muse-composer`。
- 解释知识、实验、历史或科学概念：转交 `muse-science-video` 或 `muse-lesson`。
- 15 至 90 秒、强调开头钩子和留存：转交 `muse-short-video`。
- 有角色、场景、镜头和叙事节奏：转交 `muse-filmmaker`。
- 重点是色彩、构图、粒子、镜头运动或抽象氛围：转交 `muse-visual-artist`。
- 重点是播客、访谈或声音叙事：转交 `muse-podcast`。
- 重点是关卡、预告片或游戏事件：转交 `muse-game-cinematic`。

## 共同流程

1. 先确定创作意图和验收标准，再决定 Muse 段落。
2. 用 `intent` 表达感觉，用 `orchestrate` 表达跨音乐、视觉和时间线的规则。
3. 将可执行细节落到 `timeline`、`tempo`、`music`、`visual`。
4. 创建或更新项目后运行 `muse check`，再运行 `muse preview` 检查节奏和结构。
5. 遇到不支持的素材或容器，说明替代格式，不把失败包装成成功。

不要替用户擅自决定风格、受众或平台；信息不足时只问会改变方案的最少问题。

需要收集信息时读取 [creative-brief.md](references/creative-brief.md)。
