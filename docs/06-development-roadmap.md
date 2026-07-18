# 06｜Architecture 2.0 开发路线

估算基准：一名全职核心开发者；多人可并行 Capsule，但每个 Capsule 的独立性和证据门槛不降低。

## Phase 0｜架构重置（当前）

交付：

- Architecture 2.0；
- Conversion Capsule 规范；
- Kernel/Adapter 边界；
- Capsule 与 Adapter schema；
- 自治目录模板；
- HEIC→JPEG 参考设计；
- 宇宙级 Registry 扩展路线。

退出条件：仓库中不再存在“Capsule 必须依赖 EverythingX trait/IR”的规范性要求。

## Phase 1｜Capsule Independence Kit（1–2 周）

开发：

- `check_capsule_independence.py`；
- 临时复制构建测试；
- 外部 path dependency 检查；
- capsule/adapter schema validator；
- corpus manifest 与 benchmark report schema；
- release hash 和 SBOM 约定。

退出条件：模板复制出仓库后可独立构建；删除 `everythingx/` 后检查仍通过。

## Phase 2｜三个参考 Capsule（3–5 周）

### `utf16-to-utf8`

验证 streaming、endianness、BOM、invalid sequence、替换/拒绝策略和无分配快路径。

### `bmp-to-png`

验证独立 parser/encoder、bit depth、palette、alpha、row order、压缩、质量和内存报告。

当前：0.1 原生实现已经覆盖 1/4/8 位 palette、16/32 位 bitfields、24/32 位像素、上下行、RLE4/RLE8、五类 PNG filter、分块 IDAT、CRC/Adler 与 Store/Fixed-RLE Deflate；独立 copy-out、12 个行为测试和 Kernel 默认调用已经通过 CI，下一轮转入 corpus、fuzz 和 benchmark 强化。

### `wav-pcm-to-aiff`

验证 container、endianness、sample format、metadata、streaming 和大文件边界。

当前：0.1 原生实现已经覆盖 RIFF/WAVE integer PCM 8/16/24/32 位、WAVE_FORMAT_EXTENSIBLE valid bits、8 位有符号性转换、多字节端序转换、多 data chunk、常用 LIST/INFO 元数据、AIFF 80 位采样率编码、经典 FORM/SSND 大小边界与可删除静态 Adapter；独立 copy-out、12 个行为测试和 Kernel 默认调用已经通过 CI，下一轮转入 corpus、fuzz、benchmark 和差分验证强化。

每个 Capsule 都必须：

- 独立 Cargo build/test/bench/fuzz；
- 无 EverythingX public API；
- 有完整 Options/Error/Report；
- 有可删除 Adapter；
- 有至少一个其他程序直接调用示例。

退出条件：三个 Capsule 在独立临时仓库 CI 通过。状态：已达成；随后保持独立性约束进入 Phase 4。

## Phase 3｜最小 Adapter Protocol 与薄 Kernel（已提前启动，持续 2–3 周）

以 `utf16-to-utf8` 的真实 API 为第一轮输入，只实现：

1. registry snapshot 加载；
2. Capsule/Adapter handshake；
3. 一项 capability 调用；
4. Report→effects/loss/provenance；
5. static Rust 与 process transport 各一个 reference adapter；
6. 跨进程资源上限与失败分类。

当前已建立零依赖 `ex-protocol` 和 `ex-kernel` 骨架，以及默认配置的端到端直接调用。后续两个参考 Capsule 只允许校正边界，不得借机加入多步图搜索或统一 family IR。

## Phase 4｜高价值独立库群（持续 3–6 个月）

按“用户频率 × 独立价值 × 可验证性 ÷ 复杂度”推进：

### Text/Table

```text
utf32-to-utf8
json-to-jsonl
jsonl-to-json
csv-to-jsonl
jsonl-to-csv
csv-dialect-normalize
```

### Raster

```text
png-to-jpeg
jpeg-to-png
png-to-webp
bmp-to-png
tiff-to-png
svg-render-to-png   # 明确是 render
```

### Audio

```text
wav-pcm-to-aiff
aiff-to-wav-pcm
flac-to-wav-pcm
wav-pcm-to-flac
audio-extract-channel
audio-split-time-range
```

### Package/Document

```text
zip-extract-member
tar-to-zip
pdf-extract-images
pdf-extract-attachments
pdf-split-pages
pdf-merge-pages
docx-extract-media
docx-to-document-tree
```

### Database

```text
sqlite-table-to-csv
sqlite-table-to-jsonl
csv-to-sqlite-table
sqlite-extract-blobs
sqlite-schema-to-sql
```

每个都是可单独发布的库，不是 Kernel 模块。

## Phase 5｜HEIC→JPEG Gold Capsule（分阶段 3–9 个月）

1. 自研 HEIF container/metadata 与完整损失报告；codec 先采用可替换 backend。
2. 自研 color/chroma/bit-depth/alpha/HDR pipeline。
3. 逐步实现 native HEVC still-image decoder 和 JPEG encoder。
4. SIMD、tile、并行、YCbCr 快路径和内存优化。
5. 多架构 conformance、differential、quality 和 benchmark 证据。

HEIC Capsule 是“独立、深度验证、专用直达算法”的黄金标准，不作为验证目录结构的第一个简单样例。

## Phase 6｜Planner 启动门槛

以下条件同时满足后才开发多步 Planner：

- 至少 50 个 production Capsule；
- 至少 80 个经过验证的 AdapterCapability；
- 至少 30 条真实的可组合多步路径；
- 至少存在 direct-vs-IR、native-vs-external、quality-vs-speed 对照；
- benchmark/loss 数据足以形成 Pareto 选择；
- negative knowledge 可以解释主要不可行请求；
- Universe 至少 300 个 reviewed concepts 和 500 个 operational variants。

## Phase 7｜宇宙级专业领域（长期）

- Registry 改为内容寻址分片和 append-only assertion log；
- CAD、GIS、医学、科学、EDA、固件等 Domain Pack；
- 组织私有 namespace 与本地 snapshot composition；
- 百万级 SourceRecord 和增量索引；
- Capsule federation、签名、trust 和审计网络。

## 质量 KPI

不以主仓库代码行数作为核心指标。跟踪：

```text
独立可构建 Capsule 数
production Capsule 数
conformance profile 覆盖
fuzz CPU-hours / unique regressions
benchmark 可复现率
FormatConcept/Variant 审核数
有证据与 unknown 的比例
可解释失败率
跨 Kernel 版本无需升级的 Capsule 比例
```

最重要的 KPI：删除 EverythingX 主仓库后，已发布 Capsule 仍能独立解决真实转换问题。
