# 11｜范畴物 IR 与全量算子宇宙

## 1. 为什么先列算子，再继续写转换器

此前的三个 Capsule 是用来验证独立架构的参考实现，不应继续成为施工节奏。今后的顺序改为：

```text
Format Universe 观察
  → 规范化 FormatVariant/Profile
  → 选择范畴物 IR 与关系
  → 展开候选算子
  → 逐边判断可计算性
  → 排序并实现独立 Capsule
  → AdapterCapability
  → registry/support-matrix.json
```

这样开发不会再表现为“想到一个扩展名就写一个转换”。一个族开始施工前，必须先有该族的表示空间、算子模板、候选边、负知识和明确待办。

## 2. “全量”不是手写所有扩展名排列

若有 `n` 个格式，机械书写 `n(n-1)` 个 A→B 名称仍不完整：格式有 profile、codec、容器组合、参数、复合输入、拆分/聚合和私有变体；新的格式还会持续出现。

EverythingX 对“全量”采用可计算定义：

```text
OperatorUniverse(snapshot t)
  = finite OperatorBasis
  × versioned ObjectIR set
  × reviewed FormatVariant/Profile set at t
  × arity and relation templates
  × parameter domains
```

基础是有限的，具体算子实例是开放增长的。目录生成所有类型正确的候选边，但候选边默认是 `unknown`，不能因为“能写出 A→B”就宣称可计算。

## 3. 范畴物：算子真正作用的对象

这里借用范畴论语言，但不把它装饰化：

- **物（object）**：带格式状态、能力、约束和证据的 Artifact，或某个自治 Object IR 的值；
- **态射（morphism）**：有前置条件的偏函数，即具体算子能力；
- **复合（composition）**：上一态射的输出可赋值给下一态射输入，且损失和资源约束允许；
- **恒等（identity）**：相对于明确观察层的等价，不等于字节必然相同；
- **积/余积式结构**：多输入聚合、多输出拆分和变体选择；
- **函子式桥梁**：例如 ContainerGraph→成员、AudioEvents→SampledAudio 的结构保持或语义投影。

不是所有态射都可逆。`extract` 的逆操作必须知道原关系是 `container_of`、`embeds` 还是 `packages`；`render` 和 `infer` 通常不存在真正逆态射。

## 4. Object IR 不是万能内核类型

`operators/operator-basis.json` 首轮定义 31 类自治 Object IR，覆盖：

- L0：Artifact；
- L1：ByteCoding；
- L2：ContainerGraph；
- L3：Sequence、Table、Relation、Tree、Graph、Tensor、Timeline、Scene、Package、Filesystem 等；
- L4：Text、Document、Raster、Vector、SampledAudio、AudioEvents、TimedMedia、Font、Geospatial、B-Rep 等；
- L5：Program、Formula/Workflow、Simulation；
- L6：DomainRecord。

这些 IR 是独立协议或思考模型，不是 Capsule 必须实现的 EverythingX trait。专用 A→B Capsule 可以完全不暴露 IR；只有通用 decoder、encoder 或结构算子有独立价值时，才把 IR 作为稳定边界。

## 5. 十个完备施工方向

首版有限算子基包含十族、153 个动词位置：

1. **观察与证明**：identify、probe、parse、validate、measure、compare、fingerprint；
2. **载体与边界**：frame、carve、materialize、streamify、bundle、segment；
3. **字节编码**：encode/decode、compress/decompress、encrypt/decrypt、sign/verify、error correction；
4. **语法、容器与部分**：normalize、repair、upgrade、repackage、remux、extract、embed、split、aggregate；
5. **模型代数**：project、filter、join、union、pivot、reshape、cast、merge、diff、patch、schema migrate；
6. **语义表示**：convert、render、rasterize、vectorize、sample、synthesize、transcribe、translate；
7. **信号与时间轴**：trim、resample、mix、channel map、dither、filter、time stretch、synchronize；
8. **行为与计算**：evaluate、compile、link、freeze、simulate、strip active content；
9. **推断构造**：recognize、detect、restore、source separate、OCR、ASR、generate；
10. **领域、策略与 provenance**：ontology map、unit convert、redact、anonymize、attach/verify provenance。

这十族不是格式族，而是对任意文件范畴进行施工时必须检查的操作维度。缺一类，就会系统性遗漏例如“PDF 拆附件”“数据库投影”“音频聚合声道”或“CAD 渲染”等非 1:1 转码。

生成器将 31 个 Object IR 与 153 个动词展开为 4,743 个 IR×算子研究位置，并将本体中的全部语义家族与十个算子族展开成族级研究矩阵。每个位置先标为 applicability unknown；研究的职责是把它收敛为 applicable、not-applicable、conditional 或 impossible，而不是假定 4,743 个位置都能实现。

## 6. 每个候选算子的强制问题

每条生成边进入开发排序前必须回答：

1. 输入与输出比较的是哪一层？
2. 共同语义上界或桥接 IR 是什么？
3. 保持哪些不变量？
4. 目标是否能表示输入的 channel/profile/schema/behavior？
5. 是 exact、semantic-lossless、controlled-lossy、rendered、inferential，还是 impossible？
6. 一对多/多对一的顺序、冲突、身份元和结合规则是什么？
7. metadata、provenance、active behavior 和外部引用如何处理？
8. 是否可流式，是否需 seek、随机访问或完整对象？
9. 原生实现的正确性证明与性能证据是什么？
10. 失败属于不支持、条件未满足、资源不足还是理论不可行？

## 7. 三套不可混淆的清单

| 清单 | 含义 | 权威位置 |
|---|---|---|
| Format Universe | 世界中观察到或审核过的表示 | `catalog/`、`canonical/`、`ontology/` |
| Operator backlog | 已枚举但可能尚未判断或实现的候选算子 | `operators/` |
| Implemented support | 现在确实可以运行的能力 | `registry/support-matrix.json` |

以后每次实现提交必须重建支持矩阵；计划中、研究中和不可行的边不得混进“已支持”。

## 8. 家族施工门禁

开始某一格式族的批量开发前，至少提交：

- 表示类、容器、codec/profile、复合关系和外部依赖清单；
- identify/validate/parse/convert/extract/aggregate/project/normalize/render/infer 全维度扫描；
- 由规则展开的候选边快照；
- 初始负知识与 computability review 队列；
- 连续开发波次，而不是单个孤立 Capsule 名称。

音频族是第一个执行该门禁的领域，详见 `docs/12-audio-operator-program.md`。
