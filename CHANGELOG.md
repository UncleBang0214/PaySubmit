# 更新日志

本项目遵循 [Keep a Changelog](https://keepachangelog.com/zh-CN/1.1.0/) 的书写规范，并使用 [语义化版本](https://semver.org/lang/zh-CN/)。

## [未发布]

## [0.1.1] - 2026-11-06

### 新增

- 新增「变更结算信息」页面：调用 modifyMerchantInfoV2（国密 multipart），修改子商户结算卡与结算配置。
- 设置页新增「平台商商户编号」，与国密 SM2 密钥一并保存。
- 新增「读取国密证书转 Base64」入口（移植自 YunsToolKit）。
- 结果卡支持按钮展开/收起查看服务端原始返回，便于排查。

### 调整

- 导航调整为「变更结算信息 / 余额查询 / 发起结算 / 设置」；「本地密钥」更名为「设置」并使用齿轮图标。
- 余额查询改为 MD5 签名协议；发起结算保持国密 SM2。
- 仅当 rt2_retCode=0000 视为操作成功；账户状态枚举显示为中文。

### 修复

- 响应解析兼容 GBK 编码与 form-urlencoded 文本。

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
