# EverythingX

EverythingX 是一个面向“世界上所有数字文件表示”的长期工程。它不把转换代码收编进一个万能框架，而是让每个高质量 Rust 转换库保持独立，再由 EverythingX 对这些库进行发现、适配、证明、组合与路径优化。

## 架构原则

```text
独立 Conversion Capsule
  独立构建、独立测试、独立发布、独立调用
                 ↑
        可删除的 EverythingX Adapter
                 ↑
     Registry → Kernel → Hypergraph Planner
```

依赖只能向上：

- Conversion Capsule 不得依赖 EverythingX。
- Adapter 可以依赖 Capsule 和 EverythingX Protocol。
- Kernel 只依赖协议、注册表和 Adapter，不了解转换算法内部实现。
- Planner 只读取能力、前置条件、损失、成本和证据，不拥有 codec。

一个 `heic-to-jpeg` Capsule 即使从本仓库完整复制出去，也必须能够单独 `cargo build`、运行测试、benchmark、fuzz，并作为普通 Rust library 被第三方程序调用。

## 什么是 Conversion Capsule

Capsule 是可独立交付的一项完整转换能力，例如：

```text
utf16-to-utf8
bmp-to-png
heic-to-jpeg
pdf-extract-images
sqlite-table-to-csv
```

它内部可以包含 parser、decoder、颜色处理、编码器、SIMD 和多种策略，但这些内部算法不需要服从 EverythingX 的运行时类型。EverythingX 可以同时注册：

```text
HEIC ──专用 heic-to-jpeg Capsule──> JPEG

HEIC ──decoder──> RasterIR ──encoder──> JPEG
```

专用直达实现与通用组合路径公平竞争；Planner 根据语义保持、损失、质量、速度、内存、I/O、依赖和安全选择。

## 文件哲学仍是基础

文件不是扩展名，而是信息的物质化表示：

```text
载体 → 字节编码 → 物理语法/容器 → 逻辑模型
     → 语义内容 → 行为 → 使用目的
```

同一格式可以同时属于多个范畴。PDF 可以是文档、页面图、对象容器和活动内容宿主；SQLite 可以是关系模型序列化、事务状态和复合文件集。因此 EverythingX 使用多轴本体，不建立互斥的单一分类树。

转换也不是普通有限状态机。单个 Artifact 的识别、校验、解锁、解析和验证使用状态机；拆分、聚合和 `n:m` 转换使用带类型、属性和约束的有向超图。

## 开放文件宇宙

“世界上所有文件类型”不是一个能永久封闭的有限清单。EverythingX 将它定义为随时间增长、可复现的开放宇宙：

```text
U(t) = authoritative registries
     ∪ identification registries
     ∪ operational ecosystems
     ∪ professional domain packs
     ∪ private namespaces
     ∪ unknown observations
```

当前基线保存 9,020 条来源观察，来自 IANA、PRONOM/DROID、Library of Congress FDD、Apache Tika、freedesktop shared-mime-info 和 GitHub Linguist。它们不是 9,020 个已经去重的规范格式，而是带来源、冲突和重叠的事实层。

## 仓库结构

```text
capsules/       按 domain/Object IR/operator role 分类的独立转换库；叶目录复制出去仍能构建
kernel/         EverythingX 运行时边界，只负责控制面
registry/       格式宇宙、Capsule 与 Adapter 的注册规则
operators/      Object IR、算子基、族级格式空间与可重建候选 backlog
canonical/      经证据审核的格式概念与 operational variants
catalog/        外部来源观察和索引
ontology/       多轴文件本体、关系、状态与损失词表
schemas/        SourceRecord、Format、Capsule、Adapter schema
docs/           哲学、公理、架构、治理、路线和参考设计
tools/          数据同步、目录构建和一致性校验
```

## 设计文档

建议阅读顺序：

1. `docs/00-universe-boundary.md`
2. `docs/01-file-ontology.md`
3. `docs/02-computability.md`
4. `ARCHITECTURE.md`
5. `docs/03-transform-algebra.md`
6. `docs/07-conversion-capsule.md`
7. `docs/08-kernel-and-adapters.md`
8. `docs/09-universe-scale-registry.md`
9. `docs/examples/heic-to-jpeg.md`
10. `docs/06-development-roadmap.md`
11. `docs/10-capsule-family-priority.md`
12. `docs/11-object-ir-and-operator-universe.md`
13. `docs/12-audio-operator-program.md`
14. `capsules/README.md`

## 当前阶段

现在不开发桌面端、CLI 或路径规划器。架构参考 Capsule 与首轮 PCM interchange 已完成，当前生产 Capsule 达到 22 个。本轮按完整波次一次增加 16 个：WAV 与 CAF、AU、RF64、BW64、Wave64、BWF 的双向转换，以及 4 个 raw PCM 变换。开发节奏固定为“先完成族级格式空间与全量算子 backlog，再批量实现并测试整个算子簇”。首版施工层有 31 个 Object IR、153 个算子动词、4,743 个 IR×算子研究位置和 310 个族级研究单元。音频当前归类 172 个表示、42 类操作模板和 8,672 条有序候选边，其中 16 条不同格式音频边已有实现。薄 Kernel 仍只负责注册、默认验证和直接调用。

## 当前实际支持的转换

计划数量不等于功能数量。当前可运行、经过 CI 的逻辑转换只有：

| 输入 | 输出 | 独立 Capsule | 能力 |
|---|---|---|---:|
| UTF-16 | UTF-8 | `utf16-to-utf8` | strict、replace-invalid |
| Windows BMP family | PNG | `bmp-to-png` | pixel-exact |
| RIFF/WAVE integer PCM | classic AIFF PCM | `wav-pcm-to-aiff` | pcm-exact |
| classic AIFF PCM | RIFF/WAVE integer PCM | `aiff-pcm-to-wav-pcm` | pcm-exact |
| parameterized raw integer PCM | RIFF/WAVE integer PCM | `raw-pcm-to-wav-pcm` | pcm-exact |
| RIFF/WAVE integer PCM | parameterized raw integer PCM | `wav-pcm-to-raw-pcm` | pcm-exact |
| RIFF/WAVE integer PCM | Core Audio Format PCM | `wav-pcm-to-caf-pcm` | pcm-exact |
| Core Audio Format PCM | RIFF/WAVE integer PCM | `caf-pcm-to-wav-pcm` | pcm-exact |
| RIFF/WAVE integer PCM | Sun AU/SND PCM | `wav-pcm-to-au-pcm` | pcm-exact |
| Sun AU/SND PCM | RIFF/WAVE integer PCM | `au-pcm-to-wav-pcm` | pcm-exact |
| RIFF/WAVE integer PCM | RF64 PCM | `wav-pcm-to-rf64-pcm` | pcm-exact |
| RF64 PCM | RIFF/WAVE integer PCM | `rf64-pcm-to-wav-pcm` | pcm-exact |
| RIFF/WAVE integer PCM | BW64 PCM | `wav-pcm-to-bw64-pcm` | pcm-exact |
| BW64 PCM | RIFF/WAVE integer PCM | `bw64-pcm-to-wav-pcm` | pcm-exact |
| RIFF/WAVE integer PCM | Sony Wave64 PCM | `wav-pcm-to-wave64-pcm` | pcm-exact |
| Sony Wave64 PCM | RIFF/WAVE integer PCM | `wave64-pcm-to-wav-pcm` | pcm-exact |
| RIFF/WAVE integer PCM | Broadcast WAVE PCM | `wav-pcm-to-bwf-pcm` | pcm-exact |
| Broadcast WAVE PCM | RIFF/WAVE integer PCM | `bwf-pcm-to-wav-pcm` | pcm-exact |
| parameterized raw PCM | trimmed raw PCM | `raw-pcm-trim` | frame-exact |
| parameterized raw PCM | frame-reversed raw PCM | `raw-pcm-reverse` | frame-exact |
| parameterized raw PCM | projected/reordered raw PCM channels | `raw-pcm-channel-map` | frame-exact |
| parameterized raw PCM | normalized endian/signedness raw PCM | `raw-pcm-endian-signedness-normalize` | frame-exact |

机器可读权威清单是 `registry/support-matrix.json`。任何 Capsule 或 Adapter 更新都必须运行 `python3 tools/build_support_matrix.py`；CI 会拒绝过期矩阵。计划中、研究中和不可计算的边统一保存在 `operators/`，不得写进已支持清单。

生产 Capsule 使用 `capsules/<domain>/<primary-object-ir>/<operator-role>/<capsule-name>` 层级；Schema 与 CI 会校验目录和 manifest 分类一致，并递归发现新 Capsule，因此扩展任意族类不需要继续维护手写路径列表。

运行数据校验：

```bash
python3 tools/validate_repository.py
```

从锁定上游快照重建目录：

```bash
python3 tools/sync_sources.py --destination ../research/raw
python3 tools/build_catalog.py --source-root ../research/raw
python3 tools/validate_repository.py
```

## 许可证与著作权

Required Notice: Copyright © 2026 EthanBird. All rights reserved.

本项目采用 [PolyForm Noncommercial License 1.0.0](LICENSE)，属于 **source-available（源码可见）**，不是 OSI 意义上的开源软件。任何不在公开许可范围内的商业使用必须事先取得著作权人 EthanBird 的单独书面授权，详见 `COMMERCIAL_LICENSE.md` 与 `COPYRIGHT.md`。
