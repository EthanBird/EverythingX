# 05｜架构决策记录

## ADR-001：文件使用多轴本体

同一格式可以跨语义、结构、载体、行为和领域，禁止强制唯一父类。

## ADR-002：开放文件宇宙

不宣称存在最终“全球文件格式总表”。Universe 是版本化的来源记录、断言、规范概念、专业 pack、私有 namespace 和 unknown 的并集。

## ADR-003：SourceRecord 与 FormatConcept 分离

上游记录永不自动成为内部真理；重复和冲突是知识事实。

## ADR-004：Artifact 生命周期与规划超图分离

单对象状态用状态机；split、aggregate 和 `n:m` 转换用有向超图。

## ADR-005：Conversion Capsule 是第一公民

转换实现必须能脱离 EverythingX 独立构建、测试、发布和调用。它是算法产品，不是框架插件片段。

## ADR-006：依赖反转

Capsule 不依赖 EverythingX。Adapter 依赖 Capsule 和 Protocol；Kernel 依赖 Adapter 描述。旧的强制 `EverythingXOperator` trait 方案被拒绝。

## ADR-007：Capsule 与 GraphEdge 分离

一个 Capsule 可以产生多条策略/backend 边；同一能力可以有多个 Capsule。Planner 操作 AdapterCapability，不操作 Cargo crate。

## ADR-008：Family IR 可选

公共 IR 是自治协议资产，不是 Kernel 强制类型。专用直达 Capsule 可以完全绕过 IR；跨家族语义仍须明确 render/project/infer/loss。

## ADR-009：完整转换不等于内部不可分

一个 `heic-to-jpeg` Capsule 可以内部包含 parser、decoder、color pipeline 和 encoder。是否暴露内部原语由独立产品价值决定，不由图模型强迫。

## ADR-010：可计算性是证明结果

`can_convert` 由实际输入证据、能力、不变量、损失预算、依赖、资源和信任共同求值，不是后缀矩阵。

## ADR-011：最优是 Pareto 证据

不存在脱离 corpus、硬件和质量阈值的单一最优。Capsule 提供可复现策略证据，Planner 选择 Pareto 路径。

## ADR-012：零依赖是优化目标

优先 native/portable/SIMD，但零依赖不能压过规格正确性和安全。external/system backend 必须透明、可替换并独立计入成本。

## ADR-013：Kernel 延后

先完成三个独立参考 Capsule，再从真实 API 反向设计 Adapter Protocol；不预先创建空洞的 Kernel trait 层。

## ADR-014：负知识是一等公民

Registry 保存 impossible、blocked、unsafe、not-implemented 与 unknown，确保系统能说明为什么不能算或缺少什么。

