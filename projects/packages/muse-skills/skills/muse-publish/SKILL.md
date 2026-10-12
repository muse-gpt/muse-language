---
name: muse-publish
description: 面向交付与发布准备。用户要检查、预览、导出、渲染，或为多平台准备 Muse 交付物与说明时使用。
---

# 交付工作流

## 受众

任何已有（或即将完成）`.muse` 脚本、需要验收与交付的创作者。

## 何时使用

- 说“导出、发布、交付、做预览、检查字幕/音频、多平台版本”。
- 创作 skill 已写完脚本，进入验收闸门。

脚本内容仍未成形时，先回 `muse-creator` 或对应创作 skill。

## 工作流

1. 确认交付对象、画幅、时长与目标格式（音频 / 谱面 / 预览序列 / 可能的 MP4）。
2. `muse check <script.muse>`：清空 error；逐项判断 warning 是否可接受。
3. `muse preview <script.muse>`：核对帧数、分辨率、时长、音频与 preview manifest。
4. 按用途导出：
   - 乐谱交换：`muse export <script.muse> -o out.mid --format midi` 或 `musicxml`（另有 `pdf` / `svg` / `html`）
   - 听感 / 音频：`muse render <script.muse> -o out.wav --format wav`（或 `mp3`）
   - 尝试成片：`muse render <script.muse> -o out.mp4 --format mp4`（失败则保留 preview + WAV，并写限制）
5. 需要工程脚手架时用 `muse init`；本地 skill 清单可用 `muse skill list` / `muse skill validate`；集成侧可用 `muse mcp serve`。
6. 写交付说明：脚本版本、素材侧车 `.muse.meta`、已知限制、复现命令。

## 硬约束

- 必须先 `check` 再交付；不要在仍有 error 时宣称“可以发布”。
- 不要把 PNG/WAV 预览目录重命名或伪装成 MP4/成片。
- 平台裁切必须重检主体、字幕与安全区；简单缩放不算多平台交付。
- 命令失败时粘贴真实错误与下一步，禁止伪造成功日志。
- 不写入内部排期、未公开路径或“保证上架”承诺。
