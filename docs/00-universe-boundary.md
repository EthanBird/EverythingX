# 00｜文件格式宇宙的边界

## 1. 为什么不存在一张最终的“全球文件类型大全”

文件格式没有中央造物主。标准组织、厂商、开源项目、科研仪器、游戏、数据库、固件和个人程序都能定义新的字节约定；同一个名称还可能对应多个版本、profile 或完全不同的实现。扩展名不是唯一标识，MIME 也主要是网络内容标签，而不是完整格式规范。

所以“全部”必须从静态名词改写为一个可持续的集合定义：

```text
U(t) = R_authoritative(t)
     ∪ R_identification(t)
     ∪ R_operational(t)
     ∪ R_domain(t)
     ∪ R_private(t)
     ∪ Unknown
```

其中 `t` 是快照时间。项目能够承诺的是：每次构建都能说明纳入了哪些来源、哪个版本、多少记录、如何映射，以及哪些仍然未知；不能承诺某个日期之后世界不再出现新格式。

## 2. 五种存在状态

| 状态 | 含义 | 例子 | 可作为规划依据吗 |
|---|---|---|---|
| `registered` | 在正式注册表或标准中出现 | IANA media type、ISO profile | 可以，但仍需辨别标签与格式 |
| `observed` | 工程工具或档案机构已识别 | PRONOM PUID、Tika 类型 | 可以，保留来源证据 |
| `inferred` | 从样本、魔数或结构推断 | 未登记的设备导出文件 | 仅在置信度门槛下 |
| `private` | 厂商/组织/用户命名空间 | 内部二进制快照 | 需要本地 schema 和权限 |
| `unknown` | 只有字节事实，尚无格式概念 | 未识别 blob | 只能做探测、熵分析或人工标注 |

## 3. 三层注册表，禁止直接合并

### 3.1 SourceRecord：观察事实

每条上游记录原样保留来源身份，例如 `src:pronom:fmt/18`、`src:iana:application/pdf`。它可以含名称、扩展名、MIME、签名和来源分类，但不声称自己等于 EverythingX 中的规范概念。

### 3.2 FormatConcept：规范概念

由维护者基于证据建立，例如 `exfmt:document:pdf-family`。一个概念可映射多个外部记录；映射本身是可审计断言，带关系、证据、置信度和审核者。

### 3.3 FormatVariant：可执行变体

解析与写出必须落到足够精确的版本、profile、endianness、codec/container 组合或 schema 版本，例如 `exfmt:document:pdf:2.0`、`exfmt:image:tiff:6.0:little-endian`。只有此层可以可靠地挂接解析器和序列化器。

## 4. 外部标识符不是内部主键

本库的稳定 ID 使用：

```text
exfmt:<namespace>:<slug>[:<variant>...]
src:<source>:<external-id>
exop:<domain>:<action>:<version>
```

外部标识符以 `(scheme, value)` 保存，例如：

- `media-type = application/pdf`
- `pronom-puid = fmt/276`
- `loc-fdd = fdd000277`
- `extension = pdf`

扩展名只是一种弱 signifier。冲突是正常事实，不应靠“第一个匹配”消除。

## 5. 纳入与排除原则

纳入：可序列化为有限字节、目录/包、块设备镜像或有界流的数字对象；存在可描述的语法、模型或识别证据；可以声明版本来源。

暂不纳入为“格式”：单纯应用名称；没有序列化约定的抽象数据类型；仅表示传输协议但没有持久化表示的会话；只有文件后缀却没有任何样本或规格的传闻。

流、目录、磁盘镜像和多文件工程仍可纳入，但其 `carrier` 不应被错误标成单文件。

## 6. 基线数据源的角色

| 来源 | 擅长回答 | 不应被误解为 |
|---|---|---|
| IANA Media Types | 内容标签、注册状态、结构化后缀 | 全球文件格式清单 |
| PRONOM/DROID | 格式/版本身份、扩展名、内部签名、优先级 | 语义能力模型 |
| Library of Congress FDD | 家族、组成关系、可持续性与专业格式背景 | 完整魔数数据库 |
| Apache Tika | 现实工程中的 MIME、glob、magic、继承 | 标准制定机构 |
| freedesktop shared-mime-info | 桌面环境的 glob/magic 匹配 | 跨平台唯一真相 |
| GitHub Linguist | 编程、标记、数据与 prose 文件生态 | 通用二进制格式库 |

具体版本和校验值见 `sources/sources.json`。

