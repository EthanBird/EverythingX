# 参考设计｜独立 HEIC→JPEG Capsule

此文档定义目标形态，不表示当前已经实现完整 HEVC decoder。

## 1. 外部边界

```text
Input: 受支持 HEIF/HEIC profile 的有限字节对象
Output: JPEG interchange bitstream
Result: JPEG bytes + structured Report
```

公开 API 属于 `heic-to-jpeg`，不出现 EverythingX 类型。`everythingx/adapter` 只是可选集成。

## 2. 内部流水线

```text
HEIF box parser
→ item/reference/property graph
→ primary/auxiliary image selection
→ HEVC still-image decode
→ chroma/range/bit-depth/color pipeline
→ orientation/crop/grid composition
→ alpha/HDR policy
→ JPEG sampling/quantization/entropy encode
→ EXIF/ICC/XMP migration
```

这些是 Capsule 内部原语。是否拆成独立 crate 由复用、测试和发布价值决定，不由 Planner 决定。

## 3. 必须显式处理的语义差异

JPEG 通常不能完整表达 HEIC 可能具有的：

- alpha/auxiliary image；
- HDR transfer characteristics；
- 10/12-bit 精度；
- image sequence、grid、depth 或其他 auxiliary content；
- 某些 item relationships 和 metadata。

因此 Options 必须要求策略，而不是悄悄丢弃：

```text
alpha: reject | flatten(background) | ignore
hdr: reject | tone_map(profile) | clip
bit_depth: reject | dither_to_8 | quantize_to_8
auxiliary: reject | primary_only | export_separately
metadata: strict | preserve_supported | strip
orientation: preserve_metadata | bake_pixels
```

## 4. 最优路径候选

- 避免无必要的 YCbCr→RGB→YCbCr 往返；
- 在兼容情况下匹配源和目标 chroma subsampling；
- tile/grid 流式组合，降低完整 framebuffer 峰值；
- 对 EXIF/ICC/XMP 采用经过验证的结构迁移；
- scalar、portable SIMD 和 architecture-specific SIMD 分层；
- 颜色、chroma siting、full/limited range 和 orientation 测试独立覆盖；
- 输出质量固定后再比较速度和大小。

这些是需要 benchmark 证明的假设，不是预先宣称的结论。

## 5. 实施阶段

### Stage A：容器与语义先行

自研 HEIF parser、item graph、metadata 和完整 Report；HEVC/JPEG 使用可替换 backend。目标是先证明损失模型和 corpus。

### Stage B：像素管线

自研 bit-depth、chroma、range、orientation、alpha 与 color pipeline，建立 differential 和质量测试。

### Stage C：原生 codec

逐步替换 HEVC still-image decoder 与 JPEG encoder。每个替换都必须与参考 backend 差分验证，且记录兼容 profile。

### Stage D：最优化

SIMD、tile、并行、零拷贝、内存复用和特定 profile 快路径。只有 conformance 和质量不退化时才接受性能优化。

## 6. Capsule API 报告

Report 至少返回：

```text
source profile / dimensions / bit depth / chroma
selected primary item and auxiliary handling
actual strategy/backend
color and tone-map transform
orientation handling
metadata preserved/dropped
output JPEG sampling/quality
warnings and bounded losses
timing and peak-memory measurements when requested
```

Adapter 将这些事实保守映射到 EverythingX 图边结果。Adapter 不得根据成功返回就声称 `semantic_lossless`。

