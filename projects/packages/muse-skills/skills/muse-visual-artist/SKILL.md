---
name: muse-visual-artist
description: 面向视觉、动效与氛围影像。用户要色彩、构图、材质、粒子、镜头运动或节拍响应的 Muse visual/orchestrate 规则时使用。
---

# 视觉创作工作流

## 受众

视觉艺术家、动效设计师、装置与抽象影像创作者。

## 何时使用

- 重点是画面语汇与运动，而非完整剧情或社交钩子。
- 需要把风格词落实为可复现的 `visual` / `orchestrate` 参数范围。

强叙事优先 → `muse-filmmaker`；音乐主导 → `muse-composer`。

## 工作流

1. 定义视觉语汇：主体、负空间、色温、材质、运动节奏、观看距离。风格词转换见 [visual-language.md](references/visual-language.md)。
2. `timeline` 划分视觉章节与能量变化；`tempo` 对齐节拍响应。
3. `visual` 声明背景、图像引用、`camera`、`particles` 等（视频素材须先确认工具是否支持该格式）。
4. `intent` 写 loneliness / release / tension 等感觉；`orchestrate` 映射到色温、饱和度、镜头速度、粒子与转场。
5. 为颜色、画幅、粒子密度、镜头运动设定可复现范围；记录随机种子（若使用）。
6. `muse check` → `muse preview`：检查闪烁、裁切、主体层级、循环接缝与小屏可读性。

## 硬约束

- 数值是实现手段，不能替代视觉概念；先写清“看起来像什么”，再填参数。
- 外部 AI 只生成或建议素材；编排、校验与可重复渲染由 Muse 负责。
- 保留素材侧车元数据与生成参数；不要伪造“已导入成功”。
- 预览是 PNG/WAV；不要把预览目录重命名成 MP4 交付。
