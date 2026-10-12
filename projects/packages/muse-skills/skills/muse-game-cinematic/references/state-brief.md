# 玩家状态切换表

交互床必须可重复进入；线性过场请单独标明。

## 状态清单

| 状态名 | 触发条件 | 对应 act | 循环？ | 淡入/淡出 | 禁止事项 |
| --- | --- | --- | --- | --- | --- |
| explore |  |  | 是 |  |  |
| alert |  |  | 是 |  |  |
| combat |  |  | 是 |  | 突兀跳调 |
| low_hp |  |  | 是 |  |  |
| victory |  |  | 否（线性可） |  |  |
| defeat |  |  | 否（线性可） |  |  |

## 切换边

| 从 | 到 | 音乐约束 | 视觉约束 | 备注 |
| --- | --- | --- | --- | --- |
| explore | combat |  |  |  |
| combat | victory |  |  |  |

## Muse 映射

1. `timeline`：每状态一个稳定 `act`
2. `tempo` / `music`：紧张度与动机层次
3. `orchestrate`：能量映射 + 硬约束（循环边界、淡变）
4. `muse check` → `muse preview`：从不同入口进同一 act，听/看是否跳变
5. 交换：`muse export … --format midi|musicxml`；听感：`muse render … --format wav|mp3`

说明：本表是给运行时系统的合同草稿，不是声称 Muse 内置完整游戏引擎。
