# 更新日志

本项目遵循 [Keep a Changelog](https://keepachangelog.com/zh-CN/1.1.0/) 的书写规范，并使用 [语义化版本](https://semver.org/lang/zh-CN/)。

## [未发布]

## [0.1.0] - 2026-11-05

### 新增

- 首个公开骨架版本：Tauri 2 + Rust + Vue 3 + TypeScript + Vite。
- 接口提交页面：参数编辑、提交前真实资金风险二次确认、原始/解析/请求封装三视图响应。
- 本地密钥页面：商编与签名密钥仅存本机，使用 Windows DPAPI 加密，界面只显示“已配置/未配置”。
- 内置接口目录仅保留“商户结算”（申请、查询）两个接口，协议元数据不含任何密钥与真实数据。
- 工程化配套：GitHub Issue/PR 模板、CI（前端构建 + Rust fmt/clippy/test）、Dependabot、代码规范与格式化配置。

### 安全

- 编译期默认凭据全部为空，仅保留扩展点。
- 提交请求强制 TLS 证书校验，不提供“忽略证书”开关。
- 不自动重试会再次产生资金变动的请求。

[未发布]: https://github.com/UncleBang0214/PaySubmit/compare/v0.1.0...Rust
[0.1.0]: https://github.com/UncleBang0214/PaySubmit/releases/tag/v0.1.0
