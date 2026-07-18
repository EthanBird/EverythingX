# EverythingX Architecture 2.0

状态：Accepted  
取代：以 `ex-core` trait 和共享 IR 约束所有转换实现的旧方案  
核心决策：EverythingX 是独立转换库的控制面，不是转换算法的宿主框架。

## 1. 系统目标

EverythingX 同时追求两类规模：

1. **算法规模**：持续积累能单独使用、在底层格式上反复验证的 Rust 转换库。
2. **知识规模**：持续收录全球通用、专业、私有和未知文件类型，并描述它们之间的表示、包含、兼容和转换关系。

二者必须解耦。文件宇宙可以增长到百万级观察记录而不迫使任何 Capsule 升级；某个 Capsule 可以独立迭代十年而不依赖 EverythingX Kernel 的版本。

## 2. 四个自治系统

### A. Conversion Capsule Plane

真正完成转换的数据面。每个 Capsule 是自治 Rust package 或自治的小型 workspace，拥有自己的：

- 公共 Rust API；
- parser/decoder/transform/encoder；
- Options、Error、Report；
- conformance、differential、property、roundtrip、fuzz 和 benchmark；
- 格式规格与测试语料 provenance；
- 版本、许可证和发布周期。

Capsule 不导入 EverythingX 类型，不实现 EverythingX trait，不使用仓库外部的相对路径依赖。

### B. Adapter Plane

可删除的胶水层。Adapter 把 EverythingX 的调用请求映射成 Capsule API，把 Capsule Report 映射成 EverythingX 的效果、损失和 provenance。

Adapter 可以采用：

- 静态 Rust binding；
- 独立进程协议；
- C ABI；
- WASI/WASM；
- 受控的外部工具 backend。

Capsule 不知道 Adapter 存在。

### C. Kernel and Planner Plane

纯控制面，负责：

- Registry snapshot；
- Artifact facts 与候选格式；
- 用户不变量和损失预算；
- Adapter capability handshake；
- typed directed hypergraph；
- 可计算性证明；
- 多目标路径选择；
- 调度、资源限制和 provenance 汇总。

Kernel 不实现 codec，不规定内部 IR，不读取扩展名后直接分派转换器。

### D. Format Universe Plane

独立知识系统，包含：

- immutable SourceRecord；
- canonical FormatConcept；
- operational FormatVariant/Profile；
- external identifier mappings；
- 多轴哲学 facet；
- 关系和能力断言；
- 专业领域 pack；
- 私有 namespace；
- unknown/ambiguous observation。

Universe 通过版本化 snapshot 服务 Kernel，但不成为 Capsule 的编译依赖。

## 3. 依赖律

合法依赖：

```text
Capsule                  → Rust/std、内部模块、明确第三方依赖
Adapter                  → Capsule + ex-protocol
Kernel                   → ex-protocol + registry snapshot + adapters
Planner                  → kernel facts + edge descriptors
Universe builders        → upstream snapshots + ontology schemas
```

禁止依赖：

```text
Capsule ─X→ ex-core / ex-kernel / ex-registry
Capsule ─X→ repository-relative shared implementation
Kernel  ─X→ capsule internal modules
Universe─X→ runtime codec implementation
```

## 4. 独立性判定

一个目录只有满足全部条件才能称为 Capsule：

1. 将目录复制到仓库外后可独立 `cargo build`。
2. `Cargo.toml` 不含指向 Capsule 根目录之外的 `path` dependency。
3. 删除 `everythingx/` 后核心 library、测试和 benchmark 仍工作。
4. 公共 API 中没有 EverythingX 类型。
5. 自己拥有格式错误、Options 与 Report。
6. 自己声明资源限制、安全边界和许可证。
7. 可以被一个完全不知道 EverythingX 的 Rust 程序调用。
8. Capsule 的 SemVer 不与 Kernel 版本绑定。

CI 必须通过 `tools/check_capsule_independence.py` 或等价检查来证明，而不是靠 README 声明。

## 5. Capsule 与图边不是同一实体

Capsule 是可执行产品；GraphEdge 是 Kernel 对某项能力的认识。

同一个 Capsule 可以暴露多条边：

```text
heic-to-jpeg / BestQuality  → 一条边
heic-to-jpeg / Balanced     → 一条边
heic-to-jpeg / Fast         → 一条边
```

同一条语义边也可以有多个 backend：native、SIMD、system codec、external tool。Planner 选择的是经过策略展开的 AdapterCapability，不是 Cargo crate 名称。

## 6. 内部原语与外部 Capsule

“一个 Capsule 完成一项转换”不表示它只能有一个函数。HEIC→JPEG 内部可以包含 HEIF parser、HEVC still-image decoder、color pipeline 和 JPEG encoder。

只有当内部原语本身具有稳定、独立、可复用的产品价值时才拆为自己的 Capsule。EverythingX 不为了图的漂亮而强迫算法拆分，也不为了独立性把所有实现复制成单文件。

## 7. IR 政策

Family IR 从强制架构改为可选互操作资产：

- 专用 A→B Capsule 可以使用私有 IR 或直接流式变换。
- 通用 decoder/encoder 可以选择稳定的 Raster、PCM、Table 等协议。
- IR 必须是独立版本化 crate/spec，不能归 Kernel 私有。
- Planner 可以同时比较专用直达边和通用 IR 路径。

任何跨家族投影仍须如实标记 render、infer、project 或 lossy，不因 IR 可用而改变哲学上的信息边界。

## 8. 最优算法的定义

EverythingX 不宣称存在脱离场景的单一“最快/最好”。每个 Capsule 用可复现证据描述 Pareto 面：

```text
correctness, semantic retention, visual/audio quality,
latency, throughput, peak memory, bytes written,
dependency weight, portability, energy, security
```

Capsule 可以提供多个策略；策略的 benchmark corpus、CPU、编译选项和质量度量必须公开。Planner 只比较可比证据，缺失数据记为 unknown。

## 9. 安全和信任

- Capsule 负责解析器内部的长度、递归、分配和解压边界。
- Adapter 负责沙箱、调用超时和输入输出映射。
- Kernel 负责跨步骤预算和策略执行。
- Registry 保存已知 CVE、审计、fuzz 和信任等级断言。
- 不可信输入不能因使用 native backend 而默认安全。

## 10. 演进顺序

1. 固化 Capsule 规范和独立性检查。
2. 完成三个能复制出仓库运行的参考 Capsule。
3. 用参考 Capsule 反向设计最小 Adapter Protocol。
4. 建立只会注册和调用一项能力的薄 Kernel。
5. 积累高价值独立转换库。
6. 证据与密度足够后实现 Hypergraph Planner。

架构的首要成功指标不是 EverythingX 本体代码量，而是：仓库删除后，那些转换库仍然有价值。

