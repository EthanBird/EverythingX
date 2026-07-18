# 07｜Conversion Capsule 规范

## 1. 定义

Conversion Capsule 是一个自治 Rust library 或小型 workspace，对外完成一项可以清晰命名的转换能力。它是 EverythingX 生态的最小可长期维护产品，不是 Kernel 插件源码片段。

最重要的测试是：把整个 Capsule 目录复制到另一个空仓库，它仍然有完整意义。

## 2. 标准目录

```text
capsules/<source>-to-<target>/
├── Cargo.toml
├── README.md
├── LICENSE
├── capsule.json
├── src/
│   ├── lib.rs
│   └── ...
├── tests/
│   ├── conformance.rs
│   ├── differential.rs
│   ├── properties.rs
│   └── regression.rs
├── benches/
├── fuzz/
├── corpus/
│   └── manifest.json
└── everythingx/                 # 整个目录可删除
    ├── adapter.json
    └── adapter/
        ├── Cargo.toml
        └── src/lib.rs
```

`capsule.json` 描述库本身的能力、证据和发布信息，但不是构建或运行所必需。`everythingx/` 是集成包，不属于核心 API。

## 3. 公共 API 原则

EverythingX 不规定唯一 Rust trait，但参考 API 应支持大文件、可复现策略和完整报告：

```rust
pub fn convert<R, W>(
    input: &mut R,
    output: &mut W,
    options: &Options,
) -> Result<Report, Error>
where
    R: std::io::Read + std::io::Seek,
    W: std::io::Write;
```

Capsule 自己定义：

- `Options`：质量、损失策略、资源边界、确定性要求；
- `Error`：格式无效、profile 不支持、资源上限、外部条件等；
- `Report`：识别事实、实际策略、保持/丢失内容、质量度量和警告。

`Options::default()` 必须能够直接运行。它不是配置模板，而是一套确定、文档化并有 fixture 验证的真实策略。对可能丢失 alpha、HDR、精度、关系或行为的转换，默认值优先 reject；只有不会隐藏损失时才自动选择转换策略。

禁止在公共 API 中出现 `ex_core::Artifact`、`ex_registry::FormatId` 或其他 EverythingX 类型。

## 4. 独立构建规则

- Capsule 根目录必须有自己的 `Cargo.toml`。
- 允许内部 workspace member，但所有成员必须位于 Capsule 根目录下。
- 禁止引用 Capsule 根目录外的 path dependency。
- 允许 crates.io、git、system 或 optional backend，但必须锁定并列明构建、性能与安全影响。
- 可选 CLI 只能是该 Capsule 的 `src/bin` 或 example，不能依赖 EverythingX CLI。
- 默认 feature 必须代表文档声明的可用实现，不能只有空壳。
- 所有必需参数都有可运行默认值，Capsule manifest 与 Adapter 默认映射必须一致。

## 5. “一项转换”的边界

Capsule 名称描述外部可观察能力，而不是内部函数数量。

合适：

```text
heic-to-jpeg
pdf-extract-images
sqlite-table-to-csv
wav-pcm-to-aiff
```

不合适：

```text
convert-anything
media-tools
document-utilities
```

如果一个库通过输入后缀隐藏几十种互不相关实现，它不是一个 Capsule。一个 HEIC→JPEG 库内部包含多个 codec 模块仍然是一个 Capsule，因为它们共同实现同一可观察能力。

## 6. 策略与 backend

Capsule 可以公开：

```text
BestQuality
Balanced
Fast
LowestMemory
PreserveMetadata
```

也可以有 native、SIMD、system 和 external backend。每种组合必须报告实际执行路径；不得把不同保证藏在同一个不透明 `quality=80` 后面。

## 7. 验证金字塔

生产级 Capsule 至少具备：

1. **规格一致性**：公开标准和 conformance corpus。
2. **差分测试**：与两个或更多可信实现比较，差异需解释。
3. **性质测试**：尺寸、边界、单调性、不变量。
4. **Roundtrip/观察等价**：选择正确的等价层级。
5. **回归语料**：每个 bug 都有最小复现和 hash。
6. **Fuzz**：parser、metadata、尺寸、递归和损坏输入。
7. **Benchmark**：多架构、多个文件规模、冷热启动、峰值内存和质量。
8. **安全边界**：长度、分配、递归、解压比例、输出上限。

“处理了几个样本”不构成正确性证据，“在一台机器最快”不构成最优性证据。

## 8. 最优性报告

每次 benchmark 必须绑定：

```text
capsule version
strategy/backend
compiler and flags
target CPU/features
OS
corpus manifest hash
quality/loss metrics
latency/throughput/peak memory/output size
```

只在相同语义保证和质量阈值下比较速度。通过降低质量取得的吞吐不能冒充算法更优。

## 9. 发布与版本

- Capsule 使用独立 SemVer。
- 修复 bug 且不改变保证可升 patch。
- 新增兼容策略或 backend 可升 minor。
- 输入范围、默认损失、API 或保证改变必须升 major。
- Adapter 版本与 Capsule 版本分别管理。
- Registry 记录 content hash，避免同版本内容漂移。

## 10. Definition of Done

一个 Capsule 只有满足以下条件才能进入 production registry：

- 独立性检查通过；
- public API、Options、Error、Report 完整；
- manifest 通过 schema；
- 规格和 corpus provenance 完整；
- 损失声明可验证；
- fuzz、安全与资源上限完成；
- benchmark 可复现；
- Adapter 删除后 Capsule 全部核心检查仍通过。
