# 音乐简报模板

写 `music` / `tempo` / `timeline` 前确认下列项。

## 结构

| 段落 (act) | 小节范围 | 情绪关键词 | 目标密度感 |
| --- | --- | --- | --- |
| intro |  |  |  |
| verse |  |  |  |
| chorus |  |  |  |
| bridge |  |  |  |

## 音乐参数

- 调性 `key`、拍号 `meter`、主 BPM 与变速点（对应 `tempo`）
- 核心 `motif` 及允许的变形（移调、节奏紧缩、留白）
- `template`（若使用）与主要编制设想
- 织体：`density` 仅作相对疏密，不能代替配器名单
- 和声：必须出现的终止；必须避免的进行（无法验证则标“待作曲家确认”）

## 交付区分

| 产物 | 命令示意 | 用途 |
| --- | --- | --- |
| MIDI | `muse export … --format midi` | 可编辑事件 |
| MusicXML | `muse export … --format musicxml` | 谱面交换 |
| PDF/SVG/HTML | `muse export … --format pdf\|svg\|html` | 阅读呈现 |
| WAV/MP3 | `muse render … --format wav\|mp3` | 听感 / 音频交付 |

先用自然语言写听感，再映射到 `key` / `meter` / `motif` / `template` / `density`。
