# 06｜基础代码库的发展路线

路线的单位不是“做一个支持 100 种格式的 CLI”，而是增加可验证的原子能力。每一波都先补识别/校验和 family IR，再补解析/序列化，最后补纯变换。

## Wave 0：基础设施（当前）

- 开放格式宇宙、来源记录与规范概念分层。
- 文件多轴本体、关系词表、可计算性和损失模型。
- 原子算子 schema、目录模板、构建与校验工具。
- 9,020 条来源观察作为格式发现与去重候选池。

完成标准：本仓库本身可重建、可校验；不以转换 CLI 为目标。

## Wave 1：低复杂度、无损、可建立信任的原语

优先实现：hex/base-N、checksum、UTF BOM/charset 边界、CSV/TSV dialect、JSON/JSONL、XML 基础解析、PBM/PGM/PPM、BMP 子集、WAV/RIFF PCM、ZIP/TAR 的安全列表与 byte extraction。

原因：这些格式适合验证 parser/serializer 契约、streaming、边界检查、roundtrip、fuzz 和 zero-copy；能快速形成大量可组合算子，而不先陷入复杂 codec。

## Wave 2：常用图片、音频、文档容器

- RasterIR：PNG、JPEG 家族、GIF、TIFF/WebP 按 profile 推进；颜色、alpha、bit depth 和 metadata 都进入能力/损失模型。
- AudioPCM：WAV/BWF/AIFF/FLAC 及 channel/sample 格式原语；codec 与 container 分离。
- Package/Document：ZIP、OPC/DOCX、ODF、EPUB 的成员、关系与依赖闭包；PDF 先做版本识别、对象图、附件/页面/图片提取，再做渲染或重写。

复杂 JPEG/FLAC/字体等可以先提供 external backend 清单，同时并行开发 native backend；两者必须通过同一证据套件。

## Wave 3：时基媒体与表格/数据库

- TimedMedia：MP4/Matroska 容器、track、codec、timebase、subtitle、metadata；先 remux/extract，后 transcode。
- Table/Relation：CSV/Arrow/Parquet/Excel 表模型；类型、NULL、公式、格式与关系是不同能力。
- SQLite：一致快照、schema、table/view/query projection、indexes/constraints/triggers、BLOB extraction、SQL dump；明确“数据库→一张表”和“完整数据库迁移”的差别。

## Wave 4：几何、CAD、GIS 与科学数据

- Geometry/Scene：OBJ/MTL、STL、PLY、glTF；区分 mesh、scene、materials、animation 和外部纹理闭包。
- CAD/BIM：DXF、DWG、STEP、IGES、IFC；建立 B-Rep、parametric history、assemblies、units/tolerance 能力，禁止把 raster preview 当作 CAD 转换。
- GIS：GeoJSON、Shapefile、GeoPackage、GeoTIFF；CRS/datum/axis order/topology 必须是硬约束。
- Scientific Array：HDF5、NetCDF、FITS、DICOM；切片、dataset/group、传输语法、去标识与 provenance 分开。

## Wave 5：系统与长尾专业领域

文件系统/磁盘镜像、固件、抓包、邮件与归档、EDA、财务交换、游戏资产、模型权重与 checkpoint。每个领域单独建立 IR 和安全模型，只通过显式桥梁进入通用家族。

## 什么时候开始图规划

满足以下门槛再实现 planner：

1. 至少三个 family IR 已有稳定版本；
2. 至少 50 个生产级算子清单与证据齐全；
3. 至少存在 split、aggregate、render、lossy 和 conditional-exact 各一条真实路径；
4. 成本、损失和失败分类经过 benchmark/fixture 校准；
5. 负知识能解释主要不可行请求。

Planner 第一版只读取算子清单和 Artifact facts，不负责格式探测，不偷偷调用工具，也不把 `unknown` 当作可行。

## 算子优先级评分

候选算子可按下式排队，不是路径运行时成本：

```text
priority = user_frequency × graph_connectivity × semantic_reuse × testability
         ÷ (spec_ambiguity × implementation_risk × attack_surface)
```

这会优先产生可复用的 decode/encode/normalize/extract 原语，而不是一次性 A→B 适配器。

