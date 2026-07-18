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
capsules/       独立转换库模板；复制出去仍能构建
kernel/         EverythingX 运行时边界，只负责控制面
registry/       格式宇宙、Capsule 与 Adapter 的注册规则
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

## 当前阶段

现在不开发桌面端、CLI 或路径规划器。三个零依赖参考 Capsule 已经具备独立实现：`utf16-to-utf8`、自研 parser/encoder/Deflate 的 `bmp-to-png`，以及覆盖 RIFF 扫描、PCM 字节序和有符号性、WAVE_FORMAT_EXTENSIBLE 与常用元数据的 `wav-pcm-to-aiff`。薄 Kernel 仍只支持注册、默认验证和直接调用；下一阶段优先扩充 Text/Table 独立库，并持续强化现有 Capsule 的 corpus、fuzz 与 benchmark 证据。只有当独立转换库的数量、质量和证据密度达到门槛后，才开发转换图。

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
