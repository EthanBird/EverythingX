# 12｜音频格式宇宙与全族算子计划

## 1. 当前结论

音频不能被理解成“把一种声音扩展名转成另一种”。首轮可计算目录已经纳入：

| 范畴 | 审核表示数 | 有序 A→B 候选 |
|---|---:|---:|
| 采样音频、codec 与容器 profile | 86 | 7,310 |
| MIDI、音乐事件与乐谱 | 21 | 420 |
| Tracker 与芯片/程序音乐 | 21 | 420 |
| DAW/制作工程 | 18 | 306 |
| 乐器与采样音色库 | 12 | 132 |
| 空间与沉浸式音频 | 7 | 42 |
| 播放列表、CUE 与引用 | 7 | 42 |
| **合计** | **172** | **8,672** |

此外，当前 Format Universe 索引存在 293 个 `audio/*` 媒体类型标签。IANA 同时登记文件、codec、RTP payload、冗余与 FEC 类型，因此这些标签不能直接当成 293 种可互转文件。`operators/audio/backlog.json` 完整保存标签来源、172 个已归类表示和 8,672 条有序候选边。

## 2. 音频的七类“物”

### 2.1 SampledAudio

真正的采样波形，包含：

- PCM/companded/float/DSD sample representation；
- sample rate、valid bits、channel count/layout、speaker position；
- time base、frame count、priming、padding、gapless 信息；
- loudness、cue、loop、broadcast、copyright 等 metadata。

WAV、AIFF、CAF 是容器/profile；PCM、FLAC、AAC、Opus 是 essence/coding。`WAV→MP3` 实际上是 container parse → PCM decode → perceptual encode → stream/package serialize，不是改文件头。

### 2.2 AudioEvents / Notation

MIDI 与 MusicXML 不保存同一个意义。MIDI 主要表达时间戳事件与演奏控制；MusicXML/MEI 主要表达乐谱符号、布局与记谱语义。二者可投影到共同音乐事件空间，但不天然同构。

### 2.3 Tracker / Procedural Music

MOD/XM/IT 通常组合 pattern、instrument、sample 和 playback effect；NSF/SID/SPC/PSF 可能保存命令流、程序甚至仿真状态。转为 WAV 是 `render`，不是语义无损 convert；Tracker 互转必须验证 effect、timing、instrument 和 sample 模型的可表示性。

### 2.4 Production Session

PTX、ALS、RPP、LogicX、AAF 等可能包含 clips、automation、routing、plugins、tempo、takes、external media 和设备状态。跨 DAW 不是普通转码：

- 能共同表达的 timeline/clip 可做 conditional projection；
- 缺失 plugin 或 automation model 时只能 freeze/bounce；
- 外部素材必须 materialize/consolidate；
- 专有未公开字段保持 `unknown` 或 `blocked`。

### 2.5 Instrument / Sample Bank

SF2、SFZ、DLS、Kontakt 等包含 sample、zone、key/velocity map、loop、envelope、modulation 与脚本。提取 sample 不等于转换 instrument；完整转换需要目标表达全部调制和行为。

### 2.6 Spatial Audio

ADM/BW64、Atmos Master、IAMF、MPEG-H、Ambisonics 和 SOFA 分别表达 channel、object、scene、sound field 或 HRTF 数据。它们之间可能是 convert、project 或 render，必须明确 listener/layout/renderer 假设。

### 2.7 Playlist / Cue / Reference

M3U、PLS、XSPF、CUE 等主要是外部引用与时间边界。转换要维护 URI、相对路径、编码、标题和 track 边界，不处理音频 essence 本身。

## 3. 42 类音频算子模板

首轮模板不只包含 format pair：

- identify、validate、repair、normalize；
- decode/encode、convert、remux、extract/replace stream；
- time split/concatenate、channel split/aggregate/map、mix；
- resample、bit-depth、dither、gain、loudness、filter、EQ、dynamics；
- denoise、time stretch、pitch shift、fade/crossfade、reverse、loop；
- metadata extract/map、waveform/loudness/spectrum/fingerprint；
- event/notation render、audio→event inference、source separation；
- session consolidate/bounce、instrument sample extraction、spatial render。

格式对清单覆盖“从什么到什么”；模板清单覆盖“还能对它做什么”。二者的笛卡尔展开才是音频全族待办。

## 4. 可计算性批量判定规则

所有 8,672 条候选当前默认 `unknown`，不会伪装成已支持。研究时按以下证明簇批量收敛：

### 4.1 无损 sampled-audio→无损 sampled-audio

仅当目标支持输入的 sample domain、rate、channel layout、frame count 和选定 metadata invariants 时为 `conditional_exact` 或 `semantic_lossless`。目标位宽更窄、缺少 channel/object model 或 classic chunk size 溢出时，必须拒绝或改变损失等级。

### 4.2 无损→有损

属于 `controlled_lossy`。策略必须给出 codec/profile、bitrate/quality、resampling、channel mapping、delay/padding 和质量证据。

### 4.3 有损→无损

可以精确保留“指定 decoder 产生的 sample sequence”，但不能恢复编码前信号。报告必须把 decoded-signal invariant 与 source-encoding preservation 分开。

### 4.4 有损→有损

一般存在 generation loss。只有 payload 可直接复制且目标容器兼容时，才把该路径降为 `remux` 并保持 coded essence。

### 4.5 PCM↔DSD、channel↔object、event↔waveform

通常需要 sampling、noise shaping、spatial render 或 synthesis，是 controlled-lossy/rendered；反向识别通常是 inferential。

### 4.6 工程、音色库和程序音乐

先比较模型特征集合。目标缺失特征时选择 project/freeze/render，不能用一个布尔 lossless 掩盖 plugin、脚本或 emulator behavior 丢失。

## 5. 连续开发波次

音频成为当前活动族。在达到“所有候选已按规则归类；核心可计算簇有生产实现；其余有明确 blocked/impossible/deferred 原因”之前，不再随机跳到另一个格式族。

### Wave A｜PCM interchange 闭环

优先连续完成：

```text
[done] aiff-pcm-to-wav-pcm
[done] raw-pcm-to-wav-pcm       [done] wav-pcm-to-raw-pcm
[done] wav-pcm-to-caf-pcm       [done] caf-pcm-to-wav-pcm
[done] wav-pcm-to-au-pcm        [done] au-pcm-to-wav-pcm
[done] wav-pcm-to-rf64-pcm      [done] rf64-pcm-to-wav-pcm
[done] wav-pcm-to-bwf-pcm       [done] bwf-pcm-to-wav-pcm
[done] wav-pcm-to-bw64-pcm      [done] bw64-pcm-to-wav-pcm
[done] wav-pcm-to-wave64-pcm    [done] wave64-pcm-to-wav-pcm
pcm-split-channels       pcm-aggregate-channels
[done] raw-pcm-trim       pcm-concatenate
[done] raw-pcm-channel-map
[done] raw-pcm-endian-signedness-normalize
[done] raw-pcm-reverse
```

Wave A.2 继续填充不经 WAV 中转的 PCM 容器直连网：

```text
[done] caf-pcm-to-au-pcm        [done] au-pcm-to-caf-pcm
[done] caf-pcm-to-rf64-pcm      [done] rf64-pcm-to-caf-pcm
[done] caf-pcm-to-bw64-pcm      [done] bw64-pcm-to-caf-pcm
[done] caf-pcm-to-wave64-pcm    [done] wave64-pcm-to-caf-pcm
[done] caf-pcm-to-bwf-pcm       [done] bwf-pcm-to-caf-pcm
[done] au-pcm-to-rf64-pcm       [done] rf64-pcm-to-au-pcm
[done] au-pcm-to-bw64-pcm       [done] bw64-pcm-to-au-pcm
[done] au-pcm-to-wave64-pcm     [done] wave64-pcm-to-au-pcm
[done] au-pcm-to-bwf-pcm        [done] bwf-pcm-to-au-pcm
[done] rf64-pcm-to-bw64-pcm     [done] bw64-pcm-to-rf64-pcm
```

这一批 20 个有向 Capsule 均直接读取源容器并直接写出目标容器，不把 WAV 当作隐藏的中间文件；每个 Capsule 包含 4 个核心单元测试与 1 个 Adapter 默认调用测试。

Wave A.3 完成八种 integer PCM 容器的有向直连闭环：

```text
[done] rf64-pcm-to-wave64-pcm   [done] wave64-pcm-to-rf64-pcm
[done] rf64-pcm-to-bwf-pcm      [done] bwf-pcm-to-rf64-pcm
[done] bw64-pcm-to-wave64-pcm   [done] wave64-pcm-to-bw64-pcm
[done] bw64-pcm-to-bwf-pcm      [done] bwf-pcm-to-bw64-pcm
[done] wave64-pcm-to-bwf-pcm    [done] bwf-pcm-to-wave64-pcm
[done] aiff-pcm-to-caf-pcm      [done] caf-pcm-to-aiff-pcm
[done] aiff-pcm-to-au-pcm       [done] au-pcm-to-aiff-pcm
[done] aiff-pcm-to-rf64-pcm     [done] rf64-pcm-to-aiff-pcm
[done] aiff-pcm-to-bw64-pcm     [done] bw64-pcm-to-aiff-pcm
[done] aiff-pcm-to-wave64-pcm   [done] wave64-pcm-to-aiff-pcm
[done] aiff-pcm-to-bwf-pcm      [done] bwf-pcm-to-aiff-pcm
```

至此 WAV、AIFF、CAF、AU、RF64、BW64、Wave64、BWF 的 `8 × 7 = 56` 条有向边均有独立直达 Capsule。Wave A 尚未完成的是 `1:n` / `n:1` 的 channel split、aggregate 和 concatenate，它们需要先扩展 Adapter transport，而不是伪装成单输入单输出转换。

这些仍是彼此独立的 Rust libraries；共享知识可以来自规范与测试向量，不能通过仓库外 path dependency 破坏 copy-out 独立性。

### Wave B｜Lossless codec 闭环

以 FLAC 为第一套完整自研 codec。下一批固定为 20 个 Capsule：八种 PCM 容器与 native FLAC 的 16 条双向边、native FLAC↔Ogg FLAC 两条封装边、FLAC validate 与 metadata normalize；再扩展 ALAC、WavPack、APE、TTA、TAK：

```text
wav/aiff/caf/raw PCM ↔ FLAC
native FLAC ↔ Ogg FLAC ↔ Matroska FLAC
PCM ↔ ALAC/M4A
PCM ↔ WavPack/APE/TTA/TAK
lossless metadata migration and verification
```

### Wave C｜通用容器与 essence 分离

完成 Ogg、Matroska/WebM、MP4/M4A/MOV、3GP、ASF、MXF 的 audio track identify、extract、replace、split 与 compatible remux。视频容器中的音频抽取仍归 audio/timed-media 桥接，不要求视频 decoder。

### Wave D｜主流有损 codec

依次形成 MP3、AAC、Opus、Vorbis 的 decode/encode/direct-convert 簇，再扩展 AC-3/E-AC-3、DTS、WMA、AMR/EVS 与通信 codec。每套 codec 单独经过 conformance、differential、quality 与 benchmark 门槛。

### Wave E｜信号算子

连续完成 resample、bit depth+dither、channel map、mix、trim/join、gain/loudness、filters、time/pitch 与分析算子。对不同 sample types 提供默认确定、可独立运行的 Rust API。

### Wave F｜音乐事件与程序音乐

先做 SMF 0/1/2、RMID、MIDI Clip、MusicXML/MXL/MEI 的结构操作和可计算投影；再做 MOD/XM/S3M/IT；芯片音乐先建设 parser/emulator evidence，再暴露 render。

### Wave G｜工程、音色库与空间音频

先做公开、结构清晰的 AAF/AES31/RPP、SF2/SFZ/DLS、ADM/BW64/Ambisonics；专有 DAW 与 instrument 格式保留研究、投影和 freeze/bounce 路线，不虚构完全无损互转。

## 6. 完成定义

“完成音频族”不等于 8,672 个 crate 全部写完，而是：

1. 格式/stream/profile 快照有来源且可持续扩展；
2. 所有候选边都有 computability 状态，`unknown` 下降到约定阈值；
3. 可由同一证明覆盖的边按规则批量审查；
4. 高频、独立有价值且可验证的边形成完整 Capsule 簇；
5. 不可行或专有阻塞边保留证据；
6. 每次提交自动更新实际支持矩阵，绝不把 backlog 数量当成功能数量。

## 7. 研究基线

- IANA Media Types（当前目录快照对应 2026-07）：<https://www.iana.org/assignments/media-types/media-types.xhtml>
- Library of Congress Sound formats：<https://www.loc.gov/preservation/digital/formats/fdd/sound_fdd.shtml>
- Standard MIDI Files：<https://midi.org/standard-midi-files>
- MIDI 2.0 Clip File：<https://midi.org/midi-clip-file-specification-smf-midi-2-0>
- FLAC format：<https://xiph.org/flac/format.html>
- Ogg Opus：<https://www.rfc-editor.org/rfc/rfc7845.html>

目录只把这些来源作为事实输入；是否可计算仍由 EverythingX 的不变量、表示能力和证据规则决定。
