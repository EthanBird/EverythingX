# 02｜可计算性、损失与证明义务

## 1. 转换的基本形式

原子转换是偏函数，不保证对所有输入都有定义：

```text
T : Aⁿ ⇀ Bᵐ
```

它带有前置条件 `P`、效果 `E`、不变量 `I`、损失向量 `L`、成本 `C` 与依赖集合 `D`。在 Architecture 2.0 中，实际偏函数由独立 Conversion Capsule 提供，EverythingX Adapter 负责将其声明为能力边。只有当输入 Artifact 通过识别与校验，且 `P` 成立时，该能力才可计算。

## 2. 八级可计算性判定

| 状态 | 含义 |
|---|---|
| `exact` | 字节或规范语义可逆，满足声明不变量 |
| `conditional_exact` | 在 profile、密钥、外部引用等条件下精确 |
| `semantic_lossless` | 字节不同，但任务语义保持 |
| `controlled_lossy` | 损失维度和上界已声明且在预算内 |
| `rendered` | 只保留可观察表现，结构/行为通常丢失 |
| `inferential` | 依赖启发式、模型或猜测，结果含置信度 |
| `impossible` | 目标无法表达必守不变量，或信息不可恢复 |
| `unknown` | 证据不足，不能把未知当作不可能 |

## 3. 信息守恒边界

输出信息不能无来源地超过输入：

```text
Info(output) ≤ Info(input) + Info(parameters) + Info(external_state) + Info(model_prior)
```

所谓“恢复丢失细节”“图片转可编辑 CAD”“PDF 转原始 DOCX”若没有额外先验，只能是推断或重建，不是精确反转换。Capsule API 与 Adapter 能力都必须把外部模型、用户提示、字体、schema、密钥等列为显式输入。

## 4. 能力覆盖判定

格式不是简单节点标签，而是能力集合：

```text
required = invariants(request) ∩ capabilities(actual_input)
computable iff required ⊆ capabilities(target_variant) ∪ accepted_losses
```

例：RGBA PNG 转 JPEG。若不变量包含 alpha，目标能力不覆盖，且损失预算不允许透明度丢失，则 `impossible`；若专用 Capsule 提供 `flatten(background)` 策略，或存在独立 flatten 能力且用户接受合成，则变为 `controlled_lossy`。

## 5. 损失不是一个数字

标准损失向量：

```text
L = {
  payload, structure, precision, temporal, spatial,
  color, typography, metadata, provenance,
  interactivity, behavior, relationships, security
}
```

每一维使用 `{none, normalized, bounded, unbounded, unknown}`，可附单位和上界。例如音频量化可记录位深/SNR，几何可记录 Hausdorff tolerance，图像可记录色域和 PSNR/SSIM，表格可记录类型降级与空值语义。

不要将“元数据损失”藏在一个总体质量分中。用户可能接受像素轻微变化，却不能接受 EXIF 时间或法律 provenance 丢失。

## 6. 分离的可计算条件

`extract(A → B*)` 成立，需要：

1. B 在 A 中有可定位边界或可解析的逻辑身份；
2. 解码链、密钥和外部引用可用；
3. B 能独立序列化，或提供新的封装格式；
4. 依赖闭包已处理，例如字体、纹理、关系文件；
5. 提取不是把渲染结果冒充原对象。

若原始嵌入字节可直接复制，优先 `byte_extract`；若必须解码再编码，属于 `decode_project_encode`，损失与成本不同。

## 7. 聚合的可计算条件

`aggregate(A* → B)` 不仅需要类型相同，还需声明：

- 排序规则、命名冲突和去重规则；
- schema/列类型兼容及 union/intersection 策略；
- timebase、sample rate、channel layout 或空间参考系一致性；
- 坐标单位、精度和拓扑容差；
- 元数据、provenance 与权限合并策略；
- 目标容量、索引和资源上限。

“多个 PDF 合并”“多个 CSV 纵向拼接”“多个 SQLite 合库”是三种完全不同的代数。

## 8. 证明义务

每次规划都应能给出：

```text
识别证据 → 输入有效性 → AdapterCapability 前置条件 → Capsule Report/中间状态能力
→ 目标能力覆盖 → 损失预算检查 → 成本与安全约束
```

找不到证明链时，结果是 `unknown`，不是乐观地执行。
