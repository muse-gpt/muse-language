---
name: muse-composer
description: 面向作曲家、制作人和配器者，把音乐意图发展成 Muse 曲式、和声、节拍、旋律动机和可导出的 MIDI/MusicXML/WAV/MP3。用户提到作曲、配器、和弦、旋律、配乐、乐谱或音乐情绪时使用。
---

# 作曲家工作流

先问清楚作品用途、乐器编制、情绪弧、时长、速度变化和参考作品。优先保留作曲家的选择：Muse 负责把意图、段落、节拍和跨维度规则组织起来，不替用户决定旋律风格。

## 写作顺序

1. 用 `timeline` 定义 intro、verse、chorus、bridge 等段落和小节边界。
2. 用 `tempo` 定义 BPM、段落速度和 rit.，检查速度事件是否越界或冲突。
3. 用 `music for <act>` 指定 `key`、`meter`、`motif`、`template` 和 `density`。
4. 用 `intent` 描述段落的情感，用 `orchestrate` 把张力映射到视觉或镜头规则。
5. 运行 `muse check`，再用 `muse export --format midi|musicxml` 检查乐谱交换，用 `muse render --format wav|mp3` 检查听感。

## 音乐判断

保持调性、拍号、动机和和声约束可追溯。需要 `avoid-parallel-fifths` 等规则时，确认有对应的 `harmony` 结构；无法验证的和声要求必须成为诊断。不要用无意义的密度数值掩盖“稀疏、压迫、开放”等创作意图。

MIDI 是可编辑的事件数据，MusicXML 是谱面交换数据，WAV/MP3 是听感检查和交付音频；为用户说明三者用途，不把它们混为同一种导出。

复杂项目先读取 [music-brief.md](references/music-brief.md)。
