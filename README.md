# EverythingX Format Foundation

EverythingX 的第一阶段不是“万能转换器”，而是一个可以持续生长的**文件格式事实库、哲学本体与原子算子契约**。未来的转换图、最优路径和 CLI 都应建立在这三层之上，而不是把扩展名硬编码进一串 `if/else`。

## 核心结论

文件不是一个扩展名，而是某段信息在特定层次上的物质化表示：

```text
载体 → 字节编码 → 物理语法/容器 → 逻辑模型 → 语义内容 → 使用目的
```

同一文件可以同时属于多个范畴。PDF 是文档、页面集合、图形承载器，也可能包含字体、图片、脚本和附件；SQLite 是关系模型的序列化、可变状态容器和事务日志体系。因此本库使用**多轴本体**，不用单棵互斥分类树。

转换也不是普通有限状态机。拆分、聚合和多输入/多输出要求使用**带类型、属性和约束的有向超图**：节点是信息状态，超边是原子算子，单独的 Artifact 生命周期才适合用状态机表达。

## 仓库分层

```text
外部事实层 source records
        ↓ 人工或有证据的映射
规范概念层 canonical formats
        ↓ 版本/配置/能力约束
可执行表示层 operational variants
        ↓ 原子算子
转换超图与路径规划（后续阶段）
```

- `docs/`：边界、哲学本体、可计算性、转换代数和治理规则。
- `ontology/`：机器可读的分类轴、关系和状态词表。
- `schemas/`：源记录、规范格式和算子清单的 JSON Schema。
- `sources/`：上游数据源、版本、校验值和使用边界。
- `catalog/`：由上游快照生成的事实记录和索引；它们不是自动去重后的“真理”。
- `canonical/`：经证据审核后的内部规范概念。
- `operators/`：未来每个原子转换独立存放；模板已规定边界。
- `tools/`：只用 Python 标准库的数据构建与校验工具。

## 当前基线

本快照汇集六类互补来源：IANA Media Types、PRONOM/DROID、Library of Congress FDD、Apache Tika、freedesktop shared-mime-info 与 GitHub Linguist。它们分别回答“注册了什么标签”“如何从字节识别”“格式的家族和关系”“工程上常见什么”“桌面系统怎样匹配”“源码/文本生态有哪些类型”。

“世界上所有文件类型”不是一个可封闭的有限集合：私有格式、设备固件、组织内格式和每天产生的新版本无法被任何单一注册表穷尽。本项目把它严格定义为一个**版本化、可枚举、可扩展的开放宇宙**：已注册记录 + 已观察记录 + 专业领域记录 + 私有命名空间 + 未知占位记录。

运行：

```bash
python3 tools/build_catalog.py --source-root ../research/raw
python3 tools/validate_repository.py
```

第一条命令从本地上游快照重建事实目录；第二条命令验证 JSON、NDJSON、引用完整性和 ID 唯一性。重建过程不下载网络内容，便于审计和复现。

## 设计纪律

1. MIME 类型、扩展名、魔数、格式家族、版本和 profile 永不混为同一实体。
2. 外部记录先作为不可变观察保存；相似记录不自动合并。
3. 每个算子只做一个可命名的语义动作，并显式声明输入输出数量、前置条件、损失、成本、依赖和确定性。
4. “能转换”必须有可验证的能力覆盖或明确的损失预算；不能把“能渲染成图片”说成“完整转换”。
5. 解析器、语义变换和序列化器分离；零依赖是实现策略，不是牺牲正确性的理由。

建议阅读顺序：`docs/00-universe-boundary.md` → `01-file-ontology.md` → `02-computability.md` → `03-transform-algebra.md` → `04-repository-and-governance.md`。

## 许可证与著作权

Required Notice: Copyright © 2026 EthanBird. All rights reserved.

本项目采用 [PolyForm Noncommercial License 1.0.0](LICENSE)，属于 **source-available（源码可见）**，不是 OSI 意义上的开源软件。非商业用途须遵守完整许可证；任何不在公开许可范围内的商业使用，必须事先取得著作权人 EthanBird 的单独书面授权。商业授权路径及常见场景见 `COMMERCIAL_LICENSE.md`，权利边界见 `COPYRIGHT.md`。

外部贡献在正式 CLA 流程建立前不会被合并，以保护未来商业授权所需的完整著作权链，详见 `CONTRIBUTING.md`。
