# 08｜Kernel 与 Adapter 边界

## 1. Kernel 是控制面

Kernel 只拥有跨 Capsule 的知识和策略，不拥有转换算法。

推荐模块：

```text
ex-protocol     Adapter 请求/响应与版本协商
ex-registry     snapshot、Format、Capsule、Adapter 索引
ex-facts        Artifact 候选、能力和证据
ex-policy       不变量、损失预算、依赖和安全策略
ex-runtime      adapter discovery、沙箱、资源调度
ex-planner      typed hypergraph 与 Pareto search
ex-provenance   跨步骤执行记录
```

这些是未来边界，不要求当前先写空 crate。

## 2. Kernel 不做什么

- 不解析 PNG、PDF、SQLite 或 HEIC。
- 不提供所有 Capsule 必须实现的算法 trait。
- 不强迫使用统一 IR。
- 不因扩展名相同就选择 converter。
- 不把外部进程 stderr 当结构化损失报告。
- 不修改 Capsule 的 Options 意义。

## 3. Adapter 职责

Adapter 是防腐层：

```text
EverythingX InvocationRequest
  → Capsule-native input/options
  → Capsule call
  → Capsule Report/Error
  → InvocationResult + Loss + Provenance
```

它负责：

- 类型/参数映射；
- strategy/backend 展开；
- seek、stream、temporary storage 协商；
- 超时和资源上限；
- 错误分类；
- Report 到效果/损失的保守映射；
- Capsule 与 Adapter 版本记录。

Adapter 不能夸大 Capsule 保证。无法映射的字段应为 unknown，不得静默丢弃。

## 4. 调用协议

第一版只定义语义协议，不急于锁定 transport：

```text
Handshake
  protocol_version
  capsule_id/version/hash
  adapter_id/version
  capabilities[]

InvocationRequest
  capability_id
  inputs[]
  outputs[]
  options
  invariants
  resource_budget
  trust_context

InvocationResult
  status
  outputs[]
  capsule_report
  effects
  losses
  measurements
  provenance
```

Transport 可以是 static Rust、process、C ABI 或 WASM。Protocol 语义必须稳定，transport 可以替换。

## 5. 为什么不让 Capsule 实现 Kernel trait

如果 Capsule 必须实现 `EverythingXOperator`：

- Kernel 版本会污染 Capsule SemVer；
- 复制目录后需要 EverythingX 依赖；
- 第三方难以直接使用；
- 共享 Artifact/IR 容易扩张成巨型框架；
- 算法作者被迫理解 Planner 概念。

Adapter 将这些变化隔离在集成边界。生成 adapter boilerplate 是允许的，但生成物仍依赖 Capsule 的普通 public API。

## 6. Registry 对象

Registry 分别保存：

```text
CapsuleRelease     独立库身份、版本、hash、来源和证据
AdapterRelease     适配器身份、协议、transport 和兼容范围
CapabilityBinding Capsule entry/strategy/backend 到图边的映射
FormatAssertion   输入输出格式与 capability 的证据断言
BenchmarkRecord   可复现性能和质量数据
TrustAssertion    审计、fuzz、漏洞和平台状态
```

删除某个 Adapter 不删除 Capsule；删除 Kernel 不影响 Capsule 发布；撤回一个错误断言不改写历史 snapshot。

## 7. IR 作为协议包

若 Raster、PCM、Table 等公共表示确有价值，应作为自治协议项目：

```text
ex-raster-protocol
ex-pcm-protocol
ex-table-protocol
```

它们有独立版本、兼容规则和序列化形式。Capsule可以选择依赖它们，专用直达 Capsule 也可以完全绕过。

## 8. Kernel 最小可行实现

在三个参考 Capsule 完成之前不写 Planner。为了尽早验证真实集成边界，可以先实现仅支持直接调用的薄 Kernel：

1. 载入一份 registry snapshot；
2. 验证一个 Adapter handshake；
3. 调用一个 capability；
4. 保存 Report、loss 和 provenance；
5. 证明删除 EverythingX 后 Capsule 测试不受影响。

当前实现范围限定为 `ex-protocol` 与 `ex-kernel`：注册、默认配置验证、direct capability discovery、direct invocation 和 provenance。只有三个参考 Capsule 都验证过这一薄链路后，才增加多步规划。
