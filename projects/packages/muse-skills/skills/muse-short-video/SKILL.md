---
name: muse-short-video
description: 面向竖屏与社交短视频。用户要做 15/30/60/90 秒、强调开头钩子、字幕与留存结构的 Muse 短片时使用（抖音/快手/TikTok/Reels 等）。
---

# 短视频工作流

## 受众

短视频、竖屏与社交平台创作者。

## 何时使用

- 目标时长约 15–90 秒，画幅常为 9:16（可兼 1:1 / 16:9）。
- 需要钩子、节拍剪辑、字幕安全区与多版本交付思路。

深度叙事长片 → `muse-filmmaker`；纯科普可信度优先 → `muse-science-video`。

## 工作流

1. 确认平台、目标时长、受众、是否必须无声可懂。规格不清时填 [platform-brief.md](references/platform-brief.md)。
2. 前 1–2 秒给出画面变化、问题或结果预告；每段只保留一个信息点。
3. 用 `intent` 表达吸引 / 惊讶 / 信任 / 行动；用 `orchestrate` 把能量映射到饱和度、镜头速度、粒子与转场。
4. 用 `timeline` / `tempo` 对齐 beat；`visual` 保证主体清晰、字幕不挡脸与关键动作。
5. `muse check` → `muse preview`；检查前 3 秒、中段掉点、结尾行动提示。
6. 需要音频交付时用 `muse render ... --format wav|mp3`；成片容器交给 `muse-publish` 并如实说明支持边界。

## 硬约束

- 不承诺播放量或“爆款算法”。
- 9:16 / 1:1 / 16:9 变体要分别检查构图，禁止简单居中裁切当交付。
- `muse preview` 是 PNG/WAV 预览；若最终 MP4 等容器不可用或失败，交付预览序列 + WAV，并写明转换边界。
- 无声版本与有声版本分开验收；字幕可读性属于硬要求，不是装饰。
