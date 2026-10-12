---
name: muse-creator
description: Muse 创作总入口。用户有想法、故事、情绪或参考，但尚未明确作曲、科普、短视频、影视、视觉、播客、课程或游戏方向时使用；澄清目标并路由到最合适的 Muse skill。
---

# Muse 创作入口

## 受众

想用 Muse 做视听作品、但还不确定该走哪条创作路径的人。

## 何时使用

- 需求仍是自然语言（“我想做一段……”“参考某某感觉”）。
- 形式、时长、画幅、平台尚未定案。
- 需要先选 skill，再写 `.muse`。

信息已足够指向单一方向时，直接转交对应 skill，不要重复问一轮总览问题。

## 工作流

1. 用最少必要问题整理创作简报：目的、受众、形式、时长、画幅、情绪弧、声音/视觉重点、可用素材、交付物。见 [creative-brief.md](references/creative-brief.md)。
2. 按简报路由：
   - 旋律 / 和声 / 配器 / 乐谱 → `muse-composer`
   - 科普 / 事实解释 → `muse-science-video`
   - 教学 / 培训 → `muse-lesson`
   - 15–90 秒竖屏 / 社交短视频 → `muse-short-video`
   - 叙事镜头 / 场景 / MV / 纪录 → `muse-filmmaker`
   - 色彩 / 构图 / 粒子 / 抽象动效 → `muse-visual-artist`
   - 播客 / 访谈 / 声音叙事 → `muse-podcast`
   - 关卡 / 过场 / 战斗状态 → `muse-game-cinematic`
   - 已有脚本，要检查与交付 → `muse-publish`
3. 新建工程：`muse init [path]`。
4. 写作顺序：`intent`（感觉）→ `orchestrate`（跨维度规则）→ `timeline` / `tempo` / `music` / `visual`（可执行细节）。
5. `muse check <script.muse>` 通过后再 `muse preview <script.muse>`；交付阶段交给 `muse-publish`。

## 硬约束

- 不要替用户擅自定死风格、受众或平台；只问会改变方案的最少问题。
- 不要编造 Muse 不支持的语法或命令。可用语言块：`intent`、`orchestrate`、`timeline`、`tempo`、`music`、`visual`。可用 CLI：`muse init` / `check` / `preview` / `export` / `render` / `skill` / `mcp`。
- `muse preview` 产出 PNG/WAV 预览目录，不是成片容器；失败时说明原因，不要把失败说成成功。
- 不要承诺播放量、平台推荐或“一键爆款”。
