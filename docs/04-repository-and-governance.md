# 04｜仓库、发布与治理

## 1. Monorepo 是孵化器，不是运行时边界

EverythingX 主仓库可以集中孵化 Capsule、Adapter、Registry 和文档，但每个 Capsule 必须像未来会被拆成单独仓库一样设计。

允许根 workspace 方便批量 CI；同时必须单独执行：

```bash
cargo build --manifest-path capsules/<name>/Cargo.toml
cargo test  --manifest-path capsules/<name>/Cargo.toml
```

CI 还应把 Capsule 复制到临时目录再构建，以发现偷偷引用仓库外部代码的问题。

## 2. 五类版本分别管理

| 对象 | 版本规则 |
|---|---|
| Capsule | 独立 SemVer；API、输入范围和保证属于其版本 |
| Adapter | 独立 SemVer；声明兼容的 Capsule 与 Protocol 范围 |
| Protocol | 慢速演进；协商兼容，不绑定 Kernel release |
| Universe snapshot | 内容寻址 + 上游版本/日期，历史不可重写 |
| Ontology/schema | 语义化版本；删除或改义升 major |

不得用一个 EverythingX 版本号覆盖所有转换库。

## 3. Capsule 进入主仓库

必须提交：

- 自治目录和独立 `Cargo.toml`；
- `capsule.json`；
- public API、Options、Error、Report 文档；
- 规格、corpus provenance 与许可证；
- conformance/differential/property/regression 测试；
- fuzz 配置和资源限制；
- benchmark 方法与基线；
- 可删除的 `everythingx/` 集成目录。

审核者首先在 Capsule 目录外构建，而不是先运行总 workspace。

## 4. 独立转换优先于代码复用

禁止为了减少几百行重复而让 Capsule 依赖 Kernel 私有 crate。可复用算法有三种合法去向：

1. 保留在 Capsule 内部；
2. 发布为自治、版本化、通用 Rust crate；
3. 成为另一个具有独立产品价值的 Capsule/协议包。

共享代码必须有自己的 API、版本、测试和许可证，不能成为隐藏的 monorepo 脐带。

## 5. Backend 分级

```text
native-portable  自研可移植 Rust
native-simd      自研 SIMD/architecture path
system           系统 codec/API
external         第三方 crate/library/tool
hardware         GPU/codec accelerator
```

优先发展 native 实现，但正确性、规格覆盖和安全是硬门槛。外部 backend 可以用于早期语义验证、差分 oracle 和暂时覆盖；调用必须显式记录，不能伪装成零依赖。

依赖成本按 C ABI、动态库、体积、攻击面、启动、可复现性和平台覆盖分别建模。

## 6. 贡献与著作权链

商业再许可要求清晰权利链。在正式 CLA 签署流程建立前，不合并产生独立著作权的外部代码贡献。格式事实、错误样本和资料链接也必须记录来源与再分发条件。

详见 `CONTRIBUTING.md`、`LICENSE` 与 `SOURCES_AND_LICENSING.md`。

## 7. 格式事实治理

数据同步只改变 SourceRecord snapshot。SourceRecord→FormatConcept 映射是独立审核断言，带证据、置信度和审核状态。

自动 heuristic 必须显式标记；相同 MIME、扩展名、魔数或名称不允许自动合并。专业领域通过 Domain Pack 扩展，不以修改中央 taxonomy 为唯一入口。

## 8. 安全发布

生产 Capsule release 必须：

- 锁定依赖并生成 SBOM；
- 记录构建器、target 与 artifact hash；
- 通过 fuzz 和恶意资源边界测试；
- 公开 known limitations；
- Adapter 与 Capsule artifact 分别签名/寻址；
- 可从 tag 和 corpus manifest 重现 benchmark。

## 9. 不做的事情

- 不把所有 Capsule 编译进一个巨型二进制作为唯一发布形式。
- 不要求 Capsule 跟随 Kernel 升级。
- 不通过共享 Artifact trait 侵入 Capsule API。
- 不以“步骤原子化”为理由破坏一个转换库的独立完整性。
- 不在证据不足时宣传最优、无损或全格式支持。
