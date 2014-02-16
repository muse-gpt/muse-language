---
name: muse-visual-artist
description: 面向视觉艺术家、动效设计师和装置创作者，把色彩、构图、材质、粒子、镜头运动和节拍响应转成 Muse visual/orchestrate 规则。用户提到抽象影像、动效、视觉专辑、生成艺术或沉浸式画面时使用。
---

# 视觉创作工作流

先定义视觉语汇：主体、负空间、色温、材质、运动节奏和观看距离。数值只是实现手段，不能替代视觉概念。

## 构建层次

1. 用 `timeline` 定义视觉章节和能量变化。
2. 用 `visual` 声明背景、图像/视频引用（视频素材须先确认支持范围）、camera 属性和 particles。
3. 用 `orchestrate` 将 loneliness、release、tension 等意图映射到色温、饱和度、镜头速度、粒子尺寸和转场。
4. 对颜色、画幅、粒子密度和镜头运动设置可复现的范围与随机种子。
5. 预览 PNG 序列，检查闪烁、裁切、主体层级、循环接缝和移动端可读性。

保留原始素材哈希和生成参数。外部 AI 只负责生成或建议素材，Muse 负责编排、导入、验证和可重复渲染。

需要把风格词转换成视觉参数时读取 [visual-language.md](references/visual-language.md)。
