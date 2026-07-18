# 04｜持续开发的仓库与治理

## 1. 数据先于代码，证据先于合并

上游同步只新增/更新 `SourceRecord`。任何 `SourceRecord → FormatConcept` 映射都要单独提交，并经过证据审核；不得依据相同扩展名自动合并。自动分类必须标注 `method=heuristic` 与置信度，不能伪装成人工事实。

## 2. 推荐开发顺序

1. 扩大事实覆盖：权威注册表、识别库和专业领域 registry。
2. 建立高价值 canonical concepts 与 operational variants。
3. 为单一格式实现 identify/validate/parser/serializer。
4. 为同一家族实现纯 IR 原子变换。
5. 累积 conformance corpus、roundtrip、fuzz 与 benchmark 证据。
6. 当原子算子密度足够后，再实现转换图与最优路径。

这符合“基础代码库数量先足够”的目标，也防止规划器先于真实能力而变成纸上接口。

## 3. 每个算子目录

```text
operators/<domain>.<action>/
├── operator.json       # 机器清单；图边的唯一来源
├── README.md           # 语义边界和已知限制
├── src/lib.rs          # Rust 核心；不含 CLI
├── tests/
│   ├── conformance.rs
│   ├── properties.rs
│   └── regression.rs
└── fixtures/README.md  # 样本来源、许可证、预期 hash
```

算子可以是单独 crate，后续由 workspace 聚合。核心 trait 应保持极薄；不要为了统一接口而强迫所有格式依赖一个巨型公共 IR。

## 4. 零依赖策略的边界

采用四级实现优先级：

1. `bytewise`：边界明确时直接切片、拼接、重写 header/index。
2. `native`：Rust 自研 parser/serializer，标准库或项目内小模块。
3. `system`：明确、稳定、可隔离的系统 codec/API。
4. `external`：成熟依赖或外部工具，作为可替换 backend。

“位运算最快”只在算法确实受该部分支配时成立。压缩、熵编码、色彩转换、几何布尔、字体 shaping 和复杂 codec 的正确性、SIMD、缓存局部性与算法复杂度通常比是否手写位运算更关键。所有 backend 使用同一 conformance suite 和 benchmark，按数据决定默认实现。

依赖不是布尔值，而是路径成本：依赖体积、许可证、C ABI、动态库、启动开销、攻击面和可复现性分别计量。这样既鼓励自研，又能在专业格式尚未覆盖时保留可用路径。

## 5. 版本与兼容

- Source snapshot：按获取日期与上游版本锁定，不回写历史。
- Ontology/schema：语义化版本；删除/改义需要 major。
- Canonical concept：稳定 ID 不复用；弃用用 `superseded_by`。
- Operator contract：算子语义改变即升版本；实现优化不改变语义时只升 implementation version。
- Fixtures/expected hash：与算子实现一起审查。

## 6. 新格式进入仓库的 Definition of Done

至少具备：

- 一个来源记录或可公开审计的私有 schema；
- 格式家族、表示层、信息模型和载体分类；
- 版本/profile/依赖边界；
- 至少一种可靠识别证据，或明确声明 `identification=manual-only`；
- 安全与资源风险；
- 若挂算子，输入输出、损失和测试证据完整。

## 7. 新算子的 Definition of Done

- 清单通过 schema 校验，稳定 ID 唯一；
- 单一语义动作，无隐藏格式分派；
- 前置条件和失败分类可测试；
- exact/lossy 声明有 roundtrip、oracle 或度量证据；
- 不可信输入有长度、递归、内存和解压炸弹边界；
- provenance 记录输入 hash、算子/实现版本、参数和外部依赖；
- benchmark 包含吞吐、峰值内存和输出质量，不只测 happy path。

## 8. 专业领域扩展

每个领域可增加自己的 facet、IR、能力词表和容差度量，但共享顶层公理。例如：

- SQLite/数据库：schema、constraints、indexes、transactions、views、NULL/type affinity。
- CAD/BIM：B-Rep/mesh/parametric history、units、topology、tolerance、materials。
- GIS：CRS、datum、axis order、topology、resolution。
- 医学：像素与患者/检查元数据、去标识、传输语法、监管 provenance。
- EDA：层、网表、设计规则、单位与制造约束。

专业语义无法诚实映射到通用 IR 时，保留领域 IR，并用显式投影算子连接到 table、image、geometry 等通用家族。

