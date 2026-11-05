# PaySubmit · 支付提交工具

[![CI](https://github.com/UncleBang0214/PaySubmit/actions/workflows/ci.yml/badge.svg?branch=Rust)](https://github.com/UncleBang0214/PaySubmit/actions/workflows/ci.yml)
[![License: Apache-2.0](https://img.shields.io/badge/license-Apache%202.0-blue.svg)](./LICENSE)
[![Platform](https://img.shields.io/badge/platform-Windows-0078D4.svg)](#系统要求)
[![Version](https://img.shields.io/github/v/release/UncleBang0214/PaySubmit?include_prereleases&sort=semver)](https://github.com/UncleBang0214/PaySubmit/releases)

> 一个**纯本地**的支付接口请求工具：用户自己填写商编与签名密钥，软件只把请求原样提交到你指定的支付接口，并展示原始响应。
>
> **本项目与任何支付机构无隶属、代言或合作关系，不提供支付服务，不担保交易结果。**

## 它做什么

- 按内置的渠道协议，把你在界面上填写的参数组装、签名、加密后提交。
- 展示服务端返回的原始响应、解析响应以及实际发出的请求封装，便于排查问题。
- 把商编与密钥加密保存在**本机**，界面只显示“已配置/未配置”，不回显明文。

## 它明确不做什么

- 不设任何服务端，不上传密钥、不采集设备信息、不做遥测。
- 不代收、不代付、不聚合收款、不建资金池、不做账务清算。
- 不自动重试会再次产生资金变动的请求。

## 风险声明

**提交请求可能产生真实的资金变动。** 软件只负责按你填写的内容提交，结果、金额与合规责任由操作者本人承担。请务必确认目标环境（测试/生产）与接口地址后再提交。完整免责条款见 [DISCLAIMER.md](./DISCLAIMER.md)。

## 界面预览

<!-- TODO: 发布前补充 docs/screenshot.png，并替换为：![界面预览](docs/screenshot.png) -->
> 截图将在正式发布前补充。

## 系统要求

- Windows 10 / 11（x64）。
- 需要 WebView2 运行时（Windows 11 自带；Windows 10 若缺失，安装包会自动引导安装）。
- 无需管理员权限，安装后可一键卸载，不污染系统。

## 下载

从 [Releases](https://github.com/UncleBang0214/PaySubmit/releases) 下载最新安装包（`PaySubmit_<版本>_x64-setup.exe`）。

## 技术栈

Tauri 2 + Rust + Vue 3 + TypeScript + Vite。

```text
src/                        Vue 界面（接口提交、本地密钥）
src-tauri/src/              Rust 业务：协议组装、签名加解密、本地加密存储
resources/api_catalog.json  内置接口目录（仅协议元数据，不含任何密钥）
```

## 开发

```bash
npm install
npm run tauri:dev      # 开发模式
npm run build          # 前端类型检查 + 构建
npm run lint           # 前端代码规范（ESLint）
npm run format         # 前端格式化（Prettier）
cargo test --manifest-path src-tauri/Cargo.toml
npm run tauri:build    # 打 NSIS 安装包
```

## 新增接口

内置接口目录位于 `resources/api_catalog.json`，按 `products[].interfaces[]` 组织，仅描述协议与字段元数据。
新增接口时，按现有结构追加产品与接口定义即可；若涉及新的业务域或签名组合，需同步在
`src-tauri/src/feature_api_settings.rs` 的映射中登记。提交前请确保目录中**不含任何真实商编、密钥或真实端点**。

## 安全约定

- 密钥只存本机，使用 Windows DPAPI 加密，换机失效。
- 接口只允许严格 TLS，禁止关闭证书校验。
- 开源代码里绝不包含任何真实商编或密钥，请勿向仓库提交配置文件。
- 发现安全问题请走 [SECURITY.md](./SECURITY.md) 的私密渠道，勿公开 Issue。

## 参与贡献

欢迎提交 Issue 与 PR，请先阅读 [CONTRIBUTING.md](./CONTRIBUTING.md)（DCO 签名、测试要求、禁止提交真实密钥）与 [CODE_OF_CONDUCT.md](./CODE_OF_CONDUCT.md)。

## 许可证

[Apache-2.0](./LICENSE)

---

## English

PaySubmit is a **local-only** desktop tool for submitting payment API requests on Windows. You fill in your own merchant number and signing keys; the app assembles, signs, and encrypts the request per the selected channel protocol, submits it to the endpoint you specify, and displays the raw response. Keys are stored **only on your machine** (Windows DPAPI) and are never displayed in plaintext.

**This project has no affiliation, endorsement, or partnership with any payment institution, and does not provide payment services.** Submitting a request may cause **real fund movement**; all outcomes and compliance responsibilities rest with the operator. See [DISCLAIMER.md](./DISCLAIMER.md).

Stack: Tauri 2 + Rust + Vue 3 + TypeScript + Vite. Licensed under [Apache-2.0](./LICENSE).
