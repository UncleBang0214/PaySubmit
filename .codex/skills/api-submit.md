# 接口提交维护说明

## 功能说明

将用户填写的请求参数按内置渠道协议组装、签名、加密后提交至指定支付接口，并回显原始响应与解析结果。

## 实现位置

- `src-tauri/src/feature_api_debug.rs`：接口目录加载、执行入口、HTTP 提交与响应分析
- `src-tauri/src/feature_api_protocol.rs`：各协议的请求组装、签名与响应验签
- `src-tauri/src/feature_api_crypto.rs`：MD5 / 3DES / AES / RSA / SM2 / SM4 等底层加密
- `src-tauri/src/feature_api_settings.rs`：凭据业务域与签名组合映射、目录元数据
- `src-tauri/src/core_settings.rs`：本地密钥 DPAPI 加密存储与摘要
- `src/views/ApiSubmitView.vue`：接口选择、参数编辑、结果三视图

## 命令

| action | 说明 |
|---|---|
| `api.catalog` | 返回产品与接口元数据（不含密钥） |
| `api.execute` | 单次提交请求，返回请求封装与响应 |
| `settings.get` | 返回本地密钥已配置状态摘要 |
| `settings.api.catalog` | 返回凭据业务域与签名组合目录 |
| `settings.api.save` | 保存本地密钥（值不回显） |
| `app.exit` | 退出应用（用于启动同意书“拒绝并退出”，不涉及业务数据） |

## 安全约束

- 协议组装、签名与加密只在 Rust 侧完成，前端永不接触签名密钥明文。
- 请求只允许 HTTP/HTTPS 且强制 TLS 证书校验，不提供“忽略证书”开关。
- 响应体超过上限（16 MiB）会中止读取并提示缩小范围。
- 日志与错误信息不得包含密钥、完整请求明文或完整业务响应。
- 提交前必须有真实资金风险确认框，用户确认后才发送。

## 测试

- 使用合成假密钥与本地构造的响应样例，禁止使用真实商户数据。
- 运行 `cargo test --manifest-path src-tauri/Cargo.toml` 与 `npm run build`。
- 修改协议逻辑后必须补充成功、失败与响应验签三类用例。
