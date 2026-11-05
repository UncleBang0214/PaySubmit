# 贡献指南 / Contributing

感谢你愿意为 PaySubmit 做出贡献。

## 提交前

1. 先开 Issue 讨论改动方向，避免重复劳动。
2. 从 `main` 分支切出特性分支。
3. 保证 `npm run build` 与 `cargo test --manifest-path src-tauri/Cargo.toml` 通过。
4. 新增功能需附带测试；测试只使用**合成假数据**。

## 绝对禁止

- **禁止提交任何真实商户编号、密钥、私钥、账号或密码**，包括示例配置、截图与日志。
- 禁止提交 `node_modules/`、`src-tauri/target/`、`user_settings.json` 等本地产物。
- 禁止加入任何自建后端、遥测、埋点或密钥上传逻辑。
- 禁止加入“关闭证书校验”“自动重试资金请求”这类功能。

## 提交签名（DCO）

本项目采用 DCO（Developer Certificate of Origin）。请在每次提交时签名：

```bash
git commit -s -m "修复(提交): 说明"
```

`-s` 会追加 `Signed-off-by: 你的名字 <邮箱>`，表示你确认有权贡献该代码。

## 代码风格

- Rust 遵循 `rustfmt`；TypeScript 严格类型，避免 `any`。
- 错误信息包含功能上下文与可行动原因，**不得包含密钥明文**。
- 界面对话、提示与注释使用中文；标识符使用英文。

## 许可证

贡献的代码默认以 [Apache-2.0](./LICENSE) 授权。
