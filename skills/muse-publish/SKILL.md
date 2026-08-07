---
name: muse-publish
description: 面向任何 Muse 创作者完成检查、预览、交付和平台适配。用户说“导出、发布、交付、做预览、检查字幕/音频、准备多个平台版本”时使用。
---

# 交付工作流

先确认交付对象和平台规格，再决定输出格式。交付前必须保留源脚本、素材 `.meta`、哈希、项目配置和 render manifest。

## 交付检查

1. 运行 `muse check`，解决所有 error，逐项判断 warning。
2. 运行 `muse compile --emit json` 保存稳定 IR；需要集成工具时同时保存 `--emit schema`。
3. 运行 `muse preview` 检查帧数、分辨率、时长、音频和 manifest。
4. 音乐项目按用途导出 MIDI/MusicXML 与 WAV/MP3；谱面交换和试听文件分别命名。
5. 记录当前不支持的容器或编码，不把 PNG/WAV 预览重命名成视频交付物。

交付说明应包含版本、素材哈希、随机种子、已知限制和复现命令；平台裁切必须重新检查主体、字幕和安全区。
