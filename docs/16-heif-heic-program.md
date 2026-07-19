# 16｜HEIF / HEIC 独立 Capsule 开发程序

Date: 2026-07-19  
Status: 58 个 Capsule 已完成规格化；尚未进入“已支持”矩阵。

## 1. 哲学边界

HEIF 不是像 PNG 那样的单一像素编码。它首先是建立在 ISO BMFF 工具之上的
`ir:container-graph`：文件可以表达一个主图像、多个图像、图像序列、缩略图、
alpha/depth 等辅助项、grid/overlay 派生项、变换属性以及与图像关联的 metadata。

HEIC 是 HEIF 中使用 HEVC 编码图像项的互操作 profile 家族。AVIF 使用 AV1，
虽然共享容器层，却不能被当作 HEIC 的扩展名别名。ISO/IEC 23008-12:2025 还为
HEVC、AVC、JPEG、VVC、EVC 图像或图像序列声明存储 brand；因此内部模型必须是：

```text
HEIF container graph
  + coded-image item profile
  + item/reference/property graph
  + raster or timed-media meaning
```

由此可计算性分成两层。解析、验证、抽取、集合分离和部分 metadata/属性改写可以
在不解码像素时精确完成；HEIC→PNG/JPEG 则必须进入 HEVC、颜色、位深、chroma、
alpha、HDR、orientation、grid 和 item-selection 状态机。

## 2. 后端决策

容器层使用自研、零运行时依赖的 Rust：边界检查后的 box parser、`meta` item graph、
`pitm`、`iinf/infe`、`iloc`、`iprp/ipco/ipma`、`iref`、`idat/mdat`、`irot/imir`、
grid 与 Exif/XMP 关联都属于 Capsule 自有源码。

完整 HEVC decoder/encoder 是另一个 codec 规模的状态空间。第一条完整像素生产边会
使用可替换的 `libheif-dynamic-reference` backend，以获得真实的 common-profile
支持与差分 oracle；`native-hevc-still` 只有在 corpus、conformance、颜色与质量证据
通过后才能替代默认 backend。零依赖目标不能成为把残缺 decoder 宣称为 HEIC 支持
的理由。每个像素 Capsule 仍然是独立 crate，其依赖和默认参数由自身声明，而不是
依赖 EverythingX Kernel 类型。

## 3. 连续波次

机器可读权威计划是 `operators/image/heif-heic-program.json`：

| 波次 | 数量 | 范围 |
|---|---:|---|
| H0 | 20 | 原生容器验证、item 图、抽取、metadata、集合、grid 和结构属性 |
| H1 | 20 | HEIC 与 PNG/JPEG/TIFF/BMP/TGA/QOI/PPM/PAM 的双向像素边，以及 HEVC item 和 RGBA16 桥 |
| H2 | 18 | HEIF 的 HEVC/AVC/VVC/EVC/JPEG/uncompressed profiles 与序列代数 |

H0 的 20 个 Capsule 先闭合 container-graph 基础。这些 crate 必须能复制出仓库、删除
`everythingx/` 后独立 build/test，并覆盖畸形 box、越界 extent、循环引用、属性关联、
深度与数量上限。H1 不把多图 HEIC 默认为“取第一张”；默认只接受显式 primary image，
集合和序列交给专门算子。

## 4. 可运行默认参数

计划已锁定一套保守默认值：512 MiB 输入、100,000,000 像素、1,000,000 boxes、
64 层 box 深度、100,000 items/extents、选择 primary item；目标不能表达 alpha/HDR 时
拒绝，单图转换遇到集合/序列时拒绝，raster 输出默认把 orientation 烘焙到像素，支持
的 metadata 保留，丢弃项必须进入 Report。

例如 HEIC→JPEG 的 alpha、HDR 与 10/12-bit 不能静默丢失。用户必须显式选择 flatten、
tone-map 或量化策略；默认是拒绝。这使它是一条有条件、受控有损的边，而不是伪装成
`semantic_lossless`。

## 5. 性能与图权重门槛

计划中的边不填写臆测权重。每个 Capsule 进入 production 前必须经过统一端到端
benchmark，并在自身根目录得到生成的 `edge-weight.json`。Planner 先看语义和资源
硬约束，再按具体输入大小计算延迟、峰值内存和输出大小；`load_0_to_100` 只在相同
profile 的等价边之间打破平局。

H0 的优化重点是 bounded parser、按需 box payload、extent slice 与零拷贝 item view；
H1 才比较 tile/grid 流式组合、避免不必要的 YCbCr↔RGB 往返、bit-depth/chroma 快路径、
SIMD 与并行。所有“更快”都必须先通过相同正确性和质量门槛。

## 6. 依据

- ISO/IEC 23008-12:2025：<https://www.iso.org/standard/89035.html>
- ISO/IEC 23000-22:2025 MIAF：<https://www.iso.org/standard/87576.html>
- AV1 Image File Format：<https://aomediacodec.github.io/av1-avif/>
- libheif reference implementation：<https://github.com/strukturag/libheif>
