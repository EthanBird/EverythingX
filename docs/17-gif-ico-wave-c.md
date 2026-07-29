# 17｜GIF 与 ICO/CUR Wave C

Date: 2026-07-29
Status: 29 个独立 Capsule 已实现；受控性能权重由同提交触发的 CI 生成。

## 1. 为什么把它们放在同一波

GIF、ICO 和 CUR 都是常见的低复杂度视觉载体，但它们并不等价于“一张
RGBA 图片”：

- GIF89a 可以是带时间、透明和 disposal 行为的帧序列；
- ICO 是多尺寸、多位深、多编码成员的集合；
- CUR 复用 ICO 目录，却把目录中的 planes/bit-count 字段解释为热点坐标；
- ICO/CUR 成员既可能是 PNG，也可能是没有 BMP 文件头、带 XOR/AND 双平面的 DIB。

因此 Wave C 没有把 `GIF → PNG` 写成“永远取第一帧”，也没有把
`ICO → PNG` 写成“随便取一个成员”。静态 GIF 边要求恰好一个视觉帧；
动画使用具名的帧渲染或精灵表算子。ICO/CUR 的读取边在名称中包含
`best`，按面积、位深确定性选择，并把选择写入报告。

## 2. 实现的 29 个 Capsule

### 2.1 GIF 静态网格：12 个

GIF 与 PNG、BMP、TGA、QOI、PPM、PAM 建立双向边：

```text
GIF ↔ PNG
GIF ↔ BMP
GIF ↔ TGA
GIF ↔ QOI
GIF ↔ PPM
GIF ↔ PAM
```

GIF 解码覆盖：

- GIF87a / GIF89a；
- global/local color table；
- 2–8 bit LZW minimum code size、clear/end、12-bit 字典增长和 KwKwK；
- 四遍交错；
- Graphic Control Extension 的二值透明、delay 和 disposal 0–3；
- NETSCAPE/ANIMEXTS loop count、comment 及未知数据子块的有界遍历；
- frame rectangle、palette index、子块、像素数和累计帧数的边界检查。

GIF 编码是无损域明确的精确调色板编码器：最多 256 种颜色，仅接受不透明或
二值 Alpha；透明像素的 RGB 必须为零。超出这个集合时默认拒绝，不会偷偷量化、
抖动或把半透明变成不透明。首版 LZW 通过频繁 clear code 换取状态证明简单，
压缩率不是当前最优，实际负荷会由 `edge-weight.json` 进入 Planner。

### 2.2 ICO/CUR 直接边：12 个

```text
ICO(best) → PNG / BMP / QOI
PNG / BMP / QOI → ICO(single PNG member)

CUR(best) → PNG / BMP / QOI
PNG / BMP / QOI → CUR(single PNG member)
```

读取端完整验证目录、成员范围与重叠，再确定性选择最佳成员。成员解码覆盖：

- 严格 PNG；
- BITMAPINFOHEADER 及更大兼容头；
- 1/4/8-bit palette DIB；
- 24-bit BGR DIB；
- 32-bit BGRA DIB；
- bottom-up/top-down XOR 平面；
- DWORD row padding；
- AND mask；
- “全零 Alpha 使用 AND mask”和“有效 Alpha 与 AND mask 联合”的常见 ICO 规则。

写入端生成一个 PNG-backed ICO/CUR 成员。1–256 像素目录维度被显式检查；
CUR 热点为 Capsule 参数，默认 `(0, 0)`，且必须落在图像内部。多尺寸聚合属于
后续 `n:1` collection 算子，不伪装成普通单输入边。

### 2.3 完整结构验证与动画渲染：5 个

- `validate-gif`
- `validate-ico`
- `validate-cur`
- `gif-render-frame-to-png`
- `gif-animation-to-sprite-sheet-png`

三个验证器只有在所有帧或所有成员均能完整解码后才原样提交输入字节。
帧渲染器按显式索引输出已经执行透明和 disposal 的完整画布；精灵表把所有合成帧
按时间顺序从左到右排列。两者都把时间、loop 与容器结构的不可表示损失声明为
结构/metadata loss，而不是称为无损载体转换。

## 3. 安全与独立性

每个 Capsule 都包含自己的完整 Rust 源码、`Cargo.lock`、Options、Error、Report、
测试、manifest 和可删除 Adapter。它们没有第三方依赖，也不依赖 EverythingX
Kernel 类型。

默认限制：

- 输入：512 MiB；
- 单画布或输出：100,000,000 像素；
- GIF：10,000 帧；
- ICO/CUR：1,024 成员；
- ICO/CUR 单成员写入维度：1–256。

单测直接覆盖精确调色板与二值 Alpha 往返、多帧不折叠、交错行序、32-bit DIB
Alpha/AND mask、重叠成员拒绝、截断前不输出、Adapter 默认调用和 copy-out 后构建。

## 4. 图权重

29 条新增能力必须由统一 release-mode Kernel/Adapter benchmark 测量。CI 在同一
受控 runner 上重新测量全部能力，随后：

1. 更新 `registry/performance/baseline.json`；
2. 在每个新 Capsule 根目录生成 `edge-weight.json`；
3. 把能力 ID、环境指纹、延迟、每字节 CPU、峰值内存、输出倍率和派生负荷连成证据链；
4. 恢复严格的全覆盖 freshness 门禁。

在权重生成前，不填写猜测数字，也不把计划值当成图边成本。

## 5. 下一波

常见格式仍未全部覆盖。后续顺序：

1. baseline/progressive JPEG 解码、编码、校验、metadata 与显式质量损失；
2. Classic TIFF/BigTIFF 的 IFD、strip/tile、多页与常见压缩 profile；
3. WebP lossless/lossy/animation；
4. 已规格化的 HEIF/HEIC H0 容器图 20 个 Capsule；
5. HEIC、AVIF 的真实 codec backend 像素边。

JPEG/TIFF/WebP/HEVC 不会为了保持“零依赖”而交付残缺 decoder。完整状态空间超过
本地可证明范围时，Capsule 可以拥有明确锁定、可替换、可差分验证的 codec backend；
独立性指不依赖 EverythingX，不等于禁止合理的第三方 codec。
