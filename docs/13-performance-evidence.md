# 13｜性能证据、成本模型与图规划评分

## 1. 目的

性能不是 Capsule 的一个永恒常数。它是算法版本、策略、backend、输入规模与特征、编译器、CPU、OS 和测量方法共同决定的观察值。EverythingX 因而保存“绑定环境的性能证据”，并由受控基线生成 Capsule 自带的统一权重配置；禁止在 manifest 或权重文件中手填一个脱离上下文的速度等级。

Planner 面对能力边时使用以下顺序：

1. 先满足格式、前置条件、语义不变量、损失、安全和资源硬约束；
2. 使用原始成本模型估算具体输入规模下的延迟、峰值内存和输出字节；
3. 形成多目标 Pareto 前沿；
4. 只有候选仍等价时，才使用 0–100 性能分排序或打破平局。

性能分不能让有损边击败用户要求的无损边，也不能让缺少正确性证据的实现因为速度快而获选。

## 2. 测量边界

当前 `exbench:ci-default-v1` 测量的是：

```text
Kernel.invoke_defaults
  → Adapter 参数与资源预算处理
  → Adapter 输入缓冲/输出限制
  → Capsule 默认 strategy/backend
  → 内存输出 sink
```

这正是当前静态 Kernel 实际调用的能力边。它不同于只测 Capsule 内部循环的 microbenchmark；后者可以用于算法优化，但不能直接作为图边的端到端成本。

每个能力使用确定性、合法且可重建的两个输入规模：

- small：约 16 KiB，估计固定开销与短任务延迟；
- large：约 4 MiB，估计线性吞吐、输出比例与工作内存；
- BMP 使用 64×64 与 1024×1024 的 24-bit BI_RGB 图像；
- PNG 使用 64×64 与 1024×1024 的合法 RGB8、stored-Deflate、filter-none 图像；
- PCM 容器使用双声道、48 kHz、signed 16-bit PCM；
- raw PCM 使用可同时满足单声道和双声道默认值的帧对齐输入；
- UTF-16 使用带 BOM 的有效 BMP 字符序列。

每次执行包含 2 次预热、11 次 small 样本、7 次 large 样本，并记录 p50/p95。一次同进程 4 MiB `Vec` clone 作为机器校准；它只削弱共享 runner 的机器差异，不能消除调度、频率和缓存噪声。

## 3. 原始性能参数

每个 AdapterCapability 保存：

```text
small/large input_bytes
small/large output_bytes
small/large p50_micros
small/large p95_micros
large throughput_mib_s
reported_peak_memory_bytes
```

`reported_peak_memory_bytes` 来自 Adapter/Capsule 的显式工作内存报告，不等于操作系统 RSS。未来加入独立进程或 WASI backend 时，可以增加 RSS、page fault、CPU time 与能耗观测，但必须使用新的 profile ID，不能悄悄改变旧基线含义。

## 4. Planner 成本模型

用 small/large 两点拟合首版线性模型：

```text
estimated_time_us(N)
  = fixed_latency_micros
  + nanoseconds_per_input_byte × N / 1000

estimated_peak_memory(N)
  = peak_memory_bytes_per_input_byte × N

estimated_output_bytes(N)
  = output_bytes_per_input_byte × N
```

该模型只适用于 benchmark 声明的输入族和已覆盖规模区间。压缩率敏感、分辨率超线性、递归结构、随机访问或多输入算子必须增加特征维度，不能继续假设单变量线性。

顺序路径的延迟可相加；峰值内存要结合中间 Artifact 生命周期求最大 live set，不能简单相加；输出字节用于估计 I/O 与中间存储。并行分支和 n:m 超边需要调度模型后再组合。

## 5. 0–100 派生分

首版分数组成：

| 分量 | 权重 | 含义 |
|---|---:|---|
| throughput | 55% | large 吞吐相对同轮内存复制校准值 |
| latency | 20% | small p50，相对 1 ms 软尺度 |
| memory | 15% | 显式工作内存相对输入大小 |
| stability | 10% | large p95/p50 离散程度 |

总分使用加权几何平均：

```text
score = 100 × exp(Σ weightᵢ × ln(max(componentᵢ, 1) / 100))
```

几何平均使一个极差分量不能被另一个分量完全掩盖。分数只能在相同 `profile_id`、harness hash 和环境类别中比较。Planner 应优先使用原始成本模型；总分是 UI 摘要和等价边的次级排序依据。

## 6. Capsule 自带的统一图边权重

`registry/performance/baseline.json` 是受控测量的归档源；`tools/sync_edge_weights.py`
按 Capsule 和 capability 拆分该基线，在每个生产 Capsule 根目录生成
`edge-weight.json`。因此把一个 Capsule 复制出 EverythingX 仓库后，性能向量仍与其
源码、默认 strategy/backend 和证据身份一起存在。

每条 capability 的统一配置包含：

```text
profile_id / harness_sha256 / environment fingerprint
fixed_latency_micros
nanoseconds_per_input_byte
peak_memory_bytes_per_input_byte
output_bytes_per_input_byte
performance_score_0_to_100
load_0_to_100                 # 100 - performance score；越高负荷越大
small / large 原始观测
```

`load_0_to_100` 是便于 UI 和同 profile 等价边排序的派生标量，不是唯一权重。
Planner 读取 `registry/support-matrix.json` 中 capability 的 `edge_weight` 引用后，必须
按具体输入 `N` 计算延迟、峰值内存和输出大小三维向量。语义损失和硬约束不属于这个
性能向量，也不能被更低负荷覆盖。

`schemas/edge-weight.schema.json` 固定配置形状。生成器还验证 baseline 的 Capsule ID、
Capability ID、strategy、backend、source format 与 Adapter 完全一致；104 个文件中
共有 105 条权重，因为 `utf16-to-utf8` 的两个策略是两条不同图边。

## 7. 可复现性与覆盖门槛

`tools/build_performance_harness.py` 递归发现所有生产 Adapter，生成 benchmark crate 的依赖与注册清单。CI 拒绝生成结果漂移，因此新增 Capsule 不可能默默逃过性能评估。

`tools/benchmark_capsules.py`：

- release 模式一次编译全部 Adapter；
- 校验 Capsule 与 capability 数量、ID 唯一性和执行成功；
- 生成符合 `schemas/performance-report.schema.json` 的报告；
- 记录 Rust 编译器、OS、架构、commit、runner image、fixture/harness hash；
- 输出机器可读原始成本与派生分。

当前门槛是全部 104 个生产 Capsule、105 个 AdapterCapability 必须参与；UTF-16 Capsule 的 strict 与 replace-invalid 策略分别测量。受控基线的整体大输入吞吐中位数为 1,361.282 MiB/s，范围为 26.616–3,650.848 MiB/s；20 条 Raster Wave A 边为 129.723–290.495 MiB/s，20 条 PNG Wave B 能力为 26.616–91.577 MiB/s。

CI 同时执行 `tools/sync_edge_weights.py --check` 与权重单元测试；基线、Adapter、
Capsule 版本或生成规则只要有一个变化却未重建 Capsule 配置，提交就会失败。

## 8. 基线更新规则

- 正确性测试必须先于 benchmark 通过；
- 基线只能从受控 CI profile 生成，不接受开发机手填数字；
- 速度回归需同时看绝对成本、校准后成本和噪声带；
- 单次变化不直接判定回归，连续样本或专用 runner 才能升级为 release gate；
- 修改 fixtures、样本次数、评分公式或测量边界必须更换 profile ID；
- 性能优化不能改变能力边声明的语义、不变量或损失等级。

固定更新顺序：先通过正确性测试，再从受控 profile 生成并审核 baseline，然后运行
`python3 tools/sync_edge_weights.py` 和 `python3 tools/build_support_matrix.py`，最后运行
`python3 tools/validate_repository.py`。Capsule 权重是可复现的发布产物，不是第二份
人工维护的数据源。

受控 GitHub runner 可以使用 `python3 tools/benchmark_capsules.py --publish-baseline`
一次完成测量、中央基线写入和全部 Capsule 权重重建；该选项会拒绝缺少 commit、run
和 runner image 身份的本地环境。审核已有 baseline 时仍可单独运行同步命令。

## 9. 下一轮 Capsule 计划

八种 integer PCM 容器的 56 条有向直连边和 PNG Wave B 已经闭合；按当前开发优先级，音频 Wave B 继续暂停，下一轮进入仍未覆盖的常见图像格式：

```text
GIF87a/GIF89a ↔ PNG（含帧、时间、disposal 语义）
ICO/CUR ↔ PNG/BMP（含多尺寸成员选择与聚合）
JPEG baseline/progressive ↔ PNG
TIFF profile families ↔ PNG
WebP lossy/lossless/animation ↔ PNG
AVIF/HEIF/HEIC item/sequence ↔ PNG
```

每个 codec 必须先声明 native/dependency backend 决策和可验证 profile，动画与多成员格式必须使用集合/时序算子，不能由单图 `convert` 静默丢弃。HEIF/HEIC 已拆成 H0/H1/H2 共 58 个明确 Capsule，见 `operators/image/heif-heic-program.json` 与 `docs/16-heif-heic-program.md`。FLAC 的 20-Capsule 计划保留到图像阶段之后。

八组双向 codec 边产生 16 个 Capsule，封装互转产生 2 个，验证与 metadata 规范化各 1 个，总计 20。FLAC decoder/encoder、CRC、Rice coding、subframe 与 frame scanner 必须是 Capsule 内完整可复制的 Rust 实现；开发期可以由生成器同步经过验证的源码，但不能增加 EverythingX 运行时依赖。所有新能力继续自动进入功能、copy-out 与性能评估。
