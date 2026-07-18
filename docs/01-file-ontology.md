# 01｜文件哲学本体

## 1. 基本命题

### 命题 A：文件是表示，不是意义本身

文件是一组可寻址字节及其边界；格式是一套解释这些字节的约定；信息模型是约定所表达的结构；语义是人或系统赋予该结构的意义。四者不得等同。

```text
Artifact(bytes, boundary, provenance)
  -- interpreted_by --> FormatVariant
  -- serializes -----> InformationModel
  -- denotes --------> SemanticContent
```

### 命题 B：格式身份是分层的

一个 DOCX 同时具有 ZIP 容器层、OPC package 层、WordprocessingML 逻辑层、文档语义层；MP4 可能只有容器身份而 codec 仍未知。转换判断必须指出比较的是哪一层。

### 命题 C：同质不是“扩展名相同”，而是存在共同语义上界

给定输入模型 `A` 和输出模型 `B`，若存在语义空间 `S`，使二者都能在给定不变量集合下投影到 `S`，则它们在该任务中同质。

```text
homogeneous(A, B | invariants I)
⇔ ∃S: project(A, I) ∈ S ∧ representable(B, S, I)
```

JPEG 与 PNG 在“二维 raster 像素”任务中同质；在“保留分层、矢量路径和编辑历史”任务中未必同质。CSV 与 SQLite 查询结果在“有序标量表”任务中同质，但整个 SQLite 数据库与单张 CSV 不同质。

### 命题 D：包含关系必须分型

“A 包含 B”至少有六种不同含义：

1. `container_of`：容器中有独立字节流，例如 ZIP 内的 PNG。
2. `embeds`：宿主语法内嵌对象，例如 PDF 内嵌字体。
3. `packages`：多个部件和关系共同构成一个对象，例如 DOCX/OPC。
4. `references`：只保存外部引用，例如 OBJ 引用 MTL/texture。
5. `serializes_component`：部件只在解析后成为逻辑子对象，例如数据库表。
6. `renders`：B 只是 A 的视图或表现，不是组成部件，例如页面截图。

只有前五类在满足边界与解码条件时可能“分离”；第六类是派生，不是提取。

## 2. 七层表示栈

| 层 | 问题 | 示例 |
|---|---|---|
| L0 载体 `carrier` | 对象边界在哪里 | file、directory、stream、block-image、file-set |
| L1 字节变换 `coding` | 字节是否压缩/加密/编码 | UTF-8、gzip、AES、base64 |
| L2 物理语法 `syntax` | 如何分块、定界和寻址 | RIFF chunks、ZIP central directory |
| L3 逻辑模型 `model` | 解析后是什么结构 | table、tree、graph、tensor、scene |
| L4 语义内容 `semantics` | 数据表达什么 | image、ledger、document、geometry |
| L5 行为 `behavior` | 是否含可执行或交互意义 | macro、script、formula、simulation |
| L6 用途 `pragmatics` | 在哪个领域、为了什么 | 医学影像、CAD、GIS、财务报表 |

转换算子应标注它主要作用于哪一层。`remux` 主要改变 L2；`transcode` 改变 L1/L2 且保留 L4；`render` 把 L3–L5 投影为新的感知内容；数据库查询在 L3/L4 上做投影。

## 3. 多轴分类，而非单棵树

每个格式/变体都可以在以下轴上取一个或多个值，完整词表见 `ontology/facets.json`。

### 本体轴（它以何种方式存在）

- `perceptual`：面向感知信号，如图像、声音、触觉。
- `symbolic`：符号按规则指称对象，如文本、表格、几何。
- `behavioral`：可执行、可求值或可交互的描述。
- `stateful`：承载可变状态、事务或历史。
- `composite`：主要职责是组合、封装或组织其他对象。

### 信息模型轴（解析后是什么）

`scalar`、`sequence`、`record-set`、`table`、`tree`、`graph`、`tensor`、`object-graph`、`event-log`、`scene`、`filesystem`、`package`、`opaque-stream-set`。

### 语义家族轴（表达什么）

文本、页面文档、raster 图像、vector 图形、音频信号、timed media、字幕、字体、tabular、关系数据库、图数据库、科学数组、地理空间、CAD/BIM、3D/scene、归档/包、源码、可执行物、文件系统/磁盘镜像、消息、网络抓包、配置、密码材料等。

### 形态轴

文本/二进制/混合；一维/二维/2.5D/三维/n 维；无时间/离散序列/采样连续/事件流；单体/复合/集合；自包含/外部依赖/混合。

### 运行性质轴

不可变快照、可变数据库、追加日志、可执行状态；确定/非确定；可流式/需 seek/需随机访问；安全/可能活动内容/加密/未知。

## 4. Artifact：规划器真正处理的对象

```text
Artifact = {
  bytes_or_members,
  carrier,
  candidate_formats[],
  selected_variant?,
  capabilities,
  metadata,
  provenance,
  trust,
  lifecycle_state
}
```

格式是类型，Artifact 是值。一个具体文件在探测前可能有多个候选类型；解析、校验、解密和依赖解析会不断收窄候选并增加能力事实。

## 5. 为什么不是一个“万能中间格式”

任何统一 IR 若要无损容纳 PDF 的页面绘制、SQLite 的事务约束、STEP 的 B-Rep、音频采样、可执行代码行为和磁盘文件系统，最终只会退化成无语义的字节袋。

可以建立**可选、自治、版本化的家族协议 + 明确桥梁**：Raster、AudioPCM、TimedMedia、DocumentTree/PageGraph、Table/Relation、Array/Tensor、Geometry/BRep/Scene、Archive/Package、AST/IR、Filesystem。但它们不是 EverythingX Kernel 的强制内部类型。

专用 A→B Conversion Capsule 可以使用私有 IR、流式状态或直接变换；通用 decoder/encoder 可以选择公共协议。跨家族能力仍必须承认语义投影，例如 `DocumentPage → RasterImage` 是 render，不是假装无损的普通 convert。

## 6. 身份、等价与兼容

- `byte_identical`：字节完全一致。
- `syntax_equivalent`：同一变体，允许不影响解析的规范化差异。
- `model_isomorphic`：解析后的逻辑模型同构。
- `semantic_equivalent(I)`：对任务不变量 `I` 等价。
- `observationally_equivalent(O)`：在观察集合 `O` 下表现相同。
- `compatible_subset_of`：A 的所有合法实例都是 B 的合法实例。

路径规划不得用一个布尔 `lossless` 代替这些层次。
