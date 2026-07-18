# 03｜转换代数：Capsule、能力边与超图

## 1. 三种不同对象

### Algorithm Primitive

Capsule 内部的算法部件，例如 bit reader、box parser、inverse transform、color conversion 或 entropy encoder。是否拆分由算法复用和验证决定，Planner 不要求看到它们。

### Conversion Capsule

能够脱离 EverythingX 独立构建、测试和调用的完整转换库。它可以执行 `1:1`、`1:n`、`n:1` 或 `n:m` 转换。

### AdapterCapability

EverythingX 对 Capsule 某个入口、策略和 backend 的机器可读认识。Planner 的超边来自 AdapterCapability，而不是来自 Capsule 的内部函数。

这三个层级不得再次合并成一个 `Operator` 概念。

## 2. 数学形式

Capsule 暴露的转换是偏函数：

```text
C_s,b : Aⁿ ⇀ Bᵐ × Report
```

其中 `s` 是策略，`b` 是 backend。Adapter 将它投影为带前置条件、效果、损失和成本的超边：

```text
E(C, s, b) = {
  input_type_expr,
  output_type_expr,
  preconditions,
  effects,
  invariants,
  loss,
  cost,
  evidence,
  invocation
}
```

Capsule 可以在不改变 API 的情况下优化内部实现；当保证或策略语义变化时才产生新的能力边版本。

## 3. 生命周期机器与规划图

单个 Artifact 使用状态机：

```text
unobserved → detected → validated → unlocked → parsed
          ↘ ambiguous   ↘ invalid    ↘ blocked
parsed → transformed → serialized → verified
```

多个 Artifact 的转换使用超图：

```text
HyperEdge(AdapterCapability): multiset<InputState>
                           → multiset<OutputState>
```

节点是 `FormatState + CapabilitySet + Constraints`。Capsule 不需要导入这些类型；Adapter 在调用前后完成映射。

## 4. 能力族

能力边仍可按语义分类，但分类不限制 Capsule 内部结构：

| 能力 | 意义 |
|---|---|
| `identify` | 字节事实产生候选格式与证据 |
| `validate` | 验证格式/profile |
| `convert` | 完整 A→B 转换 |
| `decode` / `encode` | 与可选公共表示互换 |
| `repackage` / `remux` | 保留 payload，改变容器 |
| `extract` / `split` | 分离已有部件或按规则切分 |
| `aggregate` / `join` | 按领域代数组合多个输入 |
| `project` | 选择字段、页面、通道或视图 |
| `normalize` | 规范化表示而保持声明语义 |
| `render` | 结构/行为投影为可观察表现 |
| `infer` | 依赖启发式、模型或外部先验构造信息 |

一个专用 `heic-to-jpeg` Capsule 可以在内部完成 decode、color transform 和 encode，对 Planner 暴露一条直接 `convert` 边。若内部 decoder 后来独立成产品，再额外注册 HEIC→Raster 能力，不需要重写原 Capsule。

## 5. 直达边与组合路径共存

```text
HEIC ──specialized direct Capsule──> JPEG

HEIC ──decode──> RasterProtocol ──transform──> RasterProtocol ──encode──> JPEG
```

专用边可能通过 YCbCr 快路径、tile 流式和 metadata 直迁移取得更低成本；通用路径可能具有更强可解释性和中间编辑能力。Planner 不能预设“步骤更少一定更好”或“公共 IR 一定更通用”。

## 6. 组合条件

`E₂ ∘ E₁` 只有满足以下条件才能加入候选路径：

1. 输出类型和能力可赋值给下一输入；
2. `E₁.effects` 建立 `E₂.preconditions`；
3. 累积损失不超过用户预算；
4. 外部依赖、权限和安全策略兼容；
5. 资源峰值在预算内；
6. provenance 能贯穿所有 Capsule；
7. Adapter 的传输形式不会引入未声明转换。

损失累积可能路径相关。二次 JPEG 编码、重复色域变换和几何量化不能机械相加，能力描述允许提供 composition rule 或声明 unknown。

## 7. 多目标最优

```text
minimize (
  semantic_loss,
  uncertainty,
  security_risk,
  latency,
  peak_memory,
  bytes_written,
  dependency_weight,
  energy,
  monetary_cost
)
subject to invariants, policy, resource_budget
```

Planner 先满足硬约束，再寻找 Pareto 前沿。所谓“最优算法”必须绑定策略、硬件、corpus 和度量；没有证据的性能值是 unknown。

## 8. 负知识

图中必须保存：

- `impossible_because`
- `blocked_by`
- `unsafe_for`
- `not_implemented`
- `unknown`

没有 Adapter 不等于理论不可能；没有 benchmark 不等于很慢；没有外部依赖不等于更正确。

