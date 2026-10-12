---
name: muse-composer
description: 面向作曲、制作与配器。用户要写主题、配乐、和弦、旋律、曲式或导出 MIDI/MusicXML/音频时使用；把音乐意图写成可检查的 Muse 时间线与 music 块。
---

# 作曲工作流

## 受众

作曲家、制作人、配器者，以及需要可编辑乐谱交换的创作者。

## 何时使用

- 核心目标是音乐本身（主题、配乐、动机发展、速度设计）。
- 需要 `muse export`（MIDI/MusicXML 等）或 `muse render`（WAV/MP3）听感检查。

偏镜头叙事或纯视觉时，分别转 `muse-filmmaker` / `muse-visual-artist`，本 skill 只负责音乐层。

## 工作流

1. 确认用途、编制、情绪弧、时长、BPM/速度变化与参考听感。复杂项目先填 [music-brief.md](references/music-brief.md)。
2. 用 `timeline` 定义 intro / verse / chorus / bridge 等 `act` 与小节范围。
3. 用 `tempo` 写 BPM 与段落变速；检查事件是否越界或互相冲突。
4. 用 `music for <act>` 写 `key`、`meter`、`motif`、`template`、`density`。
5. 用 `intent` 描述段落情感；需要画面联动时再写 `orchestrate`（映射张力，不替用户改旋律）。
6. 验证与导出：
   - `muse check <script.muse>`
   - `muse preview <script.muse>`（结构与时长）
   - `muse export <script.muse> -o out.mid --format midi` 或 `--format musicxml`
   - `muse render <script.muse> -o out.wav --format wav`（或 `mp3`）

## 硬约束

- 保留作曲家的调性、动机与和声选择；Muse 组织意图与段落，不擅自改风格。
- `density` 不能代替配器决策；无法验证的和声规则必须标为待确认诊断，不能 silently 忽略。
- MIDI = 可编辑事件；MusicXML = 谱面交换；WAV/MP3 = 听感。三者用途不同，命名与说明要分开。
- `muse export` 另支持 `pdf` / `svg` / `html` 谱面呈现；不要把预览 PNG 目录说成已导出乐谱。
- 命令失败时报告真实错误，不要伪造“导出成功”。
