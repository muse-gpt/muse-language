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

## 复用与多版本创作

- 用户要求架构或设计时交付设计；不要自行创建 crate、实现语法或运行发布。
- 多语言/多画幅需求先区分共享内容与局部差异。复用对象和组件、嵌套实例、参数端口与实例覆盖；普通语言或视口变化由绑定/布局适配，不复制全部 scene。
- PrefabVariant 表达基于一个 prefab 的艺术性差异；发行任务只选择入口、显式上下文与 outputs，两者不是同一种“版本”。
- 演唱语言与画面文字语言分别记录；完整版/短剪辑与横屏/竖屏分别决定。需要了解共享与差异时读 [creative-brief.md](references/creative-brief.md)。
- 上述概念不自动代表当前工具支持 prefab、VM 或发行任务语法。生成可执行源前核实安装版本、CLI 帮助或源码；只有设计时明确标识伪代码，不能将其作为通过 check 的脚本交付。

## 硬约束

- 不要替用户擅自定死风格、受众或平台；只问会改变方案的最少问题。
- 不要编造 Muse 不支持的语法或命令；按当前工具支持核实 `intent`、`orchestrate`、`timeline`、`tempo`、`music`、`visual`、`lyric` 及相关 CLI，避免把固定列表当作完整能力证明。
- `muse preview` 产出 PNG/WAV 预览目录，不是成片容器；失败时说明原因，不要把失败说成成功。
- 不要承诺播放量、平台推荐或“一键爆款”。
