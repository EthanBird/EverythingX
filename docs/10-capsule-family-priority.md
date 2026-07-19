# 10｜Capsule 族类优先级

## 排序原则

开发顺序不等于扩展名热度。优先级由下式指导，但每项分值必须有目录、corpus 或 benchmark 证据：

```text
Priority = usage_frequency
         × graph_fan_out
         × independent_value
         × verifiability
         ÷ implementation_complexity
```

当前 9,020 条来源观察中，启发式分类较集中的族包括 structured-text、source-code、raster-image、plain-text、audio-signal、document-flow/page-description、timed-media 和 archive。关系热点还显示大量记录依赖 `text/plain`、ZIP、XML、OOXML、TIFF 和 SQLite。来源观察存在重叠，因此这些数字只用于施工排序，不用于宣称全球唯一格式数量。

## 族类顺序

1. 表示基础：UTF、JSON、JSONL、CSV 与文本规范化；
2. 压缩与容器：GZIP、ZIP、TAR；
3. 无损栅格：BMP、PNG、PNM、TGA、TIFF、GIF/ICO 拆分聚合；
4. PCM 音频与字幕：WAV、AIFF、raw PCM、SRT、WebVTT；
5. 文档、表格与数据库：PDF 结构操作、OOXML 包、SQLite；
6. 时序媒体容器：MP4/MOV、Matroska/WebM、Ogg 的提取、切分和 remux；
7. 3D、GIS 与科学数组；
8. CAD/BIM、医学、EDA 与其他专业 Domain Pack；
9. 重型有损 codec 和渲染引擎在对应 parser、corpus 与损失模型成熟后推进。

## 统一施工顺序

每个族内部按以下顺序增长：

```text
identify → validate → parse → extract/project
         → normalize → exact convert
         → controlled lossy convert → aggregate
```

这不是要求一个 Capsule 同时实现所有步骤。每个可交付转换仍是独立 Rust 库，拥有自己的 Options、Error、Report、可运行默认参数和可删除 Adapter。

## 族级连续开发律

上面的族类顺序只用于选择下一活动族，不表示可以每完成一个 Capsule 就跨族跳转。一个族开始前必须完成 Object IR、格式/profile、全量候选边和负知识目录；开始后按闭环波次连续推进。

切换活动族至少满足一项：

1. 当前波次的所有计划 Capsule 均通过独立 CI；
2. 剩余边已审核为 blocked、impossible 或有证据的 deferred；
3. 新族具有显著更高的可验证价值，且切换原因写入 ADR。

当前活动族是音频。`wav-pcm-to-aiff` 不再是孤立终点，而是 PCM interchange Wave A 的第一条已实现边。

## Planner 门槛前的生产组合

第一批目标约 60 个 production Capsule，重点由文本/表格、归档、图像、音频/字幕、文档/数据库和时序容器构成。至少 50 个生产 Capsule、80 个已验证能力和 30 条真实组合路径出现之前，不依据想象中的公共 IR 开发多步 Planner。
