# 09｜宇宙级文件类型 Registry 展望

## 1. 开放宇宙而非最终清单

私有格式、科研仪器、固件、游戏资产、厂商版本和组织内部 schema 每天都在产生。EverythingX 不承诺一个永不变化的“全球总数”，而承诺每个时间点的宇宙快照可枚举、可追溯、可扩展和可否证。

```text
UniverseSnapshot(t) = SourceRecords
                    + MappingAssertions
                    + CanonicalConcepts
                    + OperationalVariants
                    + DomainPacks
                    + PrivateNamespaces
                    + UnknownObservations
```

## 2. 事实、断言与规范概念分离

- `SourceRecord`：上游在某一快照真实说了什么， immutable。
- `MappingAssertion`：某来源记录与内部概念的关系，带证据、置信度和审核状态。
- `FormatConcept`：EverythingX 审核后的家族或格式概念。
- `FormatVariant/Profile`：足以挂接识别和转换能力的可执行粒度。
- `ArtifactObservation`：具体字节样本的候选、签名和验证结果。

同扩展名、同 MIME 或同名称永不触发自动合并。

## 3. 多层命名空间

```text
src:iana:application/pdf
src:pronom:fmt/276
exfmt:document:pdf-family
exfmt:document:pdf:2.0
domain:dicom:<transfer-syntax-uid>
private:<organization>:<format>
unknown:<content-hash-prefix>
```

内部 ID 稳定且不复用；外部 ID 是可多值 signifier。

## 4. Domain Pack

专业领域不应全部塞进一个中央 taxonomy。每个 Domain Pack 可以扩展 facet、关系、能力和容差，但共享顶层公理。

预期 pack：

- media/publishing
- office/document
- database/analytics
- CAD/BIM/manufacturing
- GIS/remote-sensing
- medical/clinical
- scientific/HPC
- EDA/semiconductor
- finance/accounting
- filesystem/firmware/forensics
- games/interactive
- ML model/checkpoint

例如 CAD pack 可以定义 B-Rep、parametric history、assembly、unit、topology tolerance；医学 pack 可以定义 transfer syntax、patient/study/series、de-identification 和监管 provenance。

## 5. 从 9,020 条到百万级记录

当前单文件 NDJSON 适合基线，不适合无限增长。后续迁移方向：

```text
content-addressed source snapshots
→ source/domain/date shards
→ append-only assertion log
→ canonical snapshot build
→ immutable index segments
→ query service / embedded snapshot
```

建议分片键：

- source + snapshot version；
- domain pack；
- canonical ID prefix；
- signature family；
- extension/MIME inverted index segment。

所有派生索引都可从事实和断言重建，不作为唯一真相。

## 6. 联邦式贡献

组织和专业社区可以发布签名包、格式包和 Capsule registry，不必把所有事实合并进中央仓库。中央 Registry 只维护：

- namespace ownership；
- pack version/hash；
- schema compatibility；
- trust/review assertions；
- snapshot composition manifest。

用户可以组合：

```text
public-core
+ media-pack
+ cad-pack
+ company-private-pack
+ local-unknown-observations
```

私有格式无需上传字节即可参与本地规划。

## 7. Unknown 是一等公民

未知文件记录可以包含：

- content hash 与尺寸；
- entropy/printable ratio；
- prefix/suffix/signature fragments；
- block/chunk hypotheses；
- embedded known-format evidence；
- extension、来源设备和上下文；
- candidate formats 与置信度。

未知不等于不存在，也不等于不可转换。它可以通过后续样本、schema 或 Capsule 提交逐步升级为 private、observed 或 canonical。

## 8. 与 Capsule 的解耦

Capsule 通过稳定格式声明或 Adapter 映射进入 Registry，但不编译整个宇宙目录。Kernel 为一次规划加载相关 snapshot/index segment；Capsule 只处理它明确支持的字节范围。

新增一百万条 SourceRecord 不应导致 `heic-to-jpeg` 重新发布。修正 `heic-to-jpeg` 的格式范围断言也不应修改其算法源码。

## 9. 长期成功标准

- 能解释每条格式事实从何而来；
- 能保留冲突而不是隐藏冲突；
- 能让专业领域扩展而不破坏顶层本体；
- 能让私有格式本地参与规划；
- 能从 snapshot 重建全部索引；
- 能将格式知识更新与 Capsule 发布完全分离；
- 永远允许 unknown 存在。

