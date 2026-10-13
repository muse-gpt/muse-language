# @muse-gpt/muse-skills

Agent skill pack organized by **creator workflow** (compose, shorts, explainers, publish, …), not by compiler modules.

## Skills

| Skill | Purpose |
| --- | --- |
| `muse-composer` | Turn musical intent into form, harmony, meter, motif, and exportable score/audio |
| `muse-creator` | Clarify goals and route to the right Muse creator skill |
| `muse-filmmaker` | Organize logline, characters, scenes, and camera language into Muse timelines |
| `muse-game-cinematic` | Map player states and level beats into reusable game audiovisual timelines |
| `muse-lesson` | Structure learning goals, practice, and review into Muse lessons |
| `muse-podcast` | Design host pacing, chapters, beds, pauses, and optional video variants |
| `muse-publish` | Check, preview, deliver, and adapt Muse projects for target platforms |
| `muse-science-video` | Turn a scientific question into a credible, evidence-led Muse video structure |
| `muse-short-video` | Design 9:16 hooks, beats, captions, and multi-version short-form delivery |
| `muse-visual-artist` | Translate color, composition, particles, and camera motion into Muse visual rules |

## Example

```bash
npx @muse-gpt/muse-skills
```

```text
Use muse-composer to draft an 8-bar C-major theme as .muse, then run muse check.
```

CLI: [`@muse-gpt/muse`](https://www.npmjs.com/package/@muse-gpt/muse).  
Product overview: [repository README](https://github.com/muse-gpt/muse-language#readme).

## 复用、本地化与画幅适配

创作 skills 将共享对象/组件、嵌套实例、参数端口与局部覆盖作为组织作品的方法。PrefabVariant 表达资产的艺术性差异；发行任务选择入口、显式上下文和产物，不要求复制语言×画幅的整套工程。

演唱与显示语言分别决定，译文通过语义 cue 关联源时间。字幕、标题、字效、封面和尾卡一起检查本地化；布局按实际文字测量、视口和安全区调整，短视频剪辑与竖屏尺寸独立选择。

这些指导不构成当前 CLI 对 prefab、VM 或发行任务语法的支持声明。Skill 应先核实安装工具能力，使用可执行源和真实检查；设计示意明确标识。仅请求设计时，交付设计，不自行实现运行代码。
