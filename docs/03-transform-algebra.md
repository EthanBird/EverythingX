# 03｜转换算子代数与超图

## 1. 两个不同的机器

### Artifact 生命周期状态机

```text
unobserved → detected → validated → unlocked → parsed
          ↘ ambiguous   ↘ invalid    ↘ blocked
parsed → transformed → serialized → verified
```

这是单个对象的状态变化，适合有限状态机。

### 转换规划超图

拆分是 `1 → n`，聚合是 `n → 1`，同步音视频可能是 `n → m`；普通有向图只能别扭地伪装。因此规划层定义为带类型属性的有向超图：

```text
HyperEdge(operator): multiset<InputType> → multiset<OutputType>
```

节点不是后缀，而是 `FormatVariant + SemanticType + CapabilitySet + Constraints` 所形成的信息状态。

## 2. 原子算子族

| 算子族 | 主要作用 | 典型形态 |
|---|---|---|
| `identify` | 字节/结构 → 候选类型和证据 | 1→1 metadata |
| `validate` | 判断是否满足规范/profile | 1→1 state |
| `decode` | bytes → family IR | 1→1 |
| `encode` | family IR → bytes | 1→1 |
| `repackage` | 保留 payload，改变外壳 | 1→1 |
| `remux` | 保留 elementary streams，改变容器 | n→1 或 1→1 |
| `transcode` | 改变信号编码，尽量保留语义 | 1→1 |
| `project` | 选择字段、页面、通道、视图 | 1→1 |
| `extract` | 分离已存在的部件 | 1→n |
| `split` | 按规则切分连续/集合对象 | 1→n |
| `aggregate` | 同类对象按明确代数组合 | n→1 |
| `join` | 按键或关系连接异质模型 | n→1 |
| `normalize` | 规范化而不改变声明语义 | 1→1 |
| `render` | 结构/行为 → 可观察表现 | 1→1，通常不可逆 |
| `infer` | 由模型/启发式构造缺失信息 | n→m，含置信度 |
| `encrypt/decrypt` | 安全封装变换 | 1→1，需密钥 |

## 3. 一份代码只做一件事

每个 `operators/<id>/` 是独立可测试单元，禁止出现“根据输入后缀做所有事情”的巨型算子。例如：

```text
exop:png:decode:1
exop:raster:flatten-alpha:1
exop:jpeg:encode:1
```

PNG → JPEG 的路径由三个算子组合。这样才能分别替换自研 PNG decoder、SIMD alpha 合成或不同 JPEG encoder，也能精确计算损失和成本。

解析器、IR 变换、序列化器也必须分开。仅容器层操作若能安全复制原始字节，应提供 zero-copy 快路径，避免不必要的解码/重编码。

## 4. 算子签名

算子清单必须声明：

- 稳定 ID、版本和实现版本；
- `n:m` arity 与输入/输出类型表达式；
- 主要作用层和算子族；
- 前置条件、效果、保持不变量和损失向量；
- 确定性、可逆性、幂等性；
- streaming/seek/random-access/mmap 能力；
- CPU、内存、I/O、质量、延迟和启动成本模型；
- 外部依赖、许可证、平台与硬件能力；
- 安全边界、资源上限和不可信输入策略；
- conformance、property、roundtrip、fuzz 与 benchmark 测试证据。

机器 schema 见 `schemas/operator.schema.json`，模板见 `operators/_template/operator.json`。

## 5. 组合律不是默认成立

算子 `T₂ ∘ T₁` 仅在以下条件满足时可组合：

1. `T₁.output_type` 可赋值给 `T₂.input_type`；
2. `T₁.effects` 建立了 `T₂.preconditions`；
3. 累积损失不超过预算；
4. 外部依赖、权限和安全策略兼容；
5. 资源峰值在预算内；
6. provenance 可贯穿整条路径。

损失累积通常不是简单相加；二次 JPEG 编码、重复色域转换和坐标量化可能产生路径依赖。因此 loss 模型允许算子提供 composition rule。

## 6. 后续路径优化的目标函数

最优路径是多目标 Pareto 问题，不应只有“步骤最少”：

```text
minimize (semantic_loss, uncertainty, security_risk,
          wall_time, peak_memory, bytes_written,
          dependency_weight, energy, monetary_cost)
subject to invariants, resource_budget, policy
```

默认先满足硬约束，再按用户策略选择 Pareto 前沿上的路径。零依赖、自研实现和 zero-copy 可分别进入 `dependency_weight`、`trust` 与 I/O 成本，而不应压过语义正确性。

## 7. 图中需要一等公民的负知识

除了可行边，还要记录：

- `impossible_because`：目标缺少某能力；
- `blocked_by`：缺密钥、字体、schema 或外部部件；
- `unsafe_for`：解析器不接受不可信输入；
- `not_implemented`：理论可行但暂无算子；
- `unknown`：尚未证明。

这使系统能解释“为什么不能算”和“缺什么才能算”，而不是简单报 unsupported。

