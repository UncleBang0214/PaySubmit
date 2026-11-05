# 安全策略 / Security Policy

## 报告漏洞

请**不要**通过公开 Issue 报告安全漏洞。

请通过以下方式私下联系维护者：

- 在 GitHub 使用 **Report a vulnerability**（[Security Advisories](https://github.com/UncleBang0214/PaySubmit/security/advisories/new)）私密通道；
- 或通过仓库维护者在 GitHub 主页公开的联系方式私下联系。

请尽量提供：

- 受影响的版本号与操作系统；
- 复现步骤或最小复现样例；
- 影响评估（密钥泄露、请求伪造、数据外泄等）。

我们会在确认后尽快修复并在发布说明中致谢（如你同意）。

## 安全边界

- 本软件为纯本地工具，不存在自建服务端，因此不存在“服务端漏洞”这一类风险面。
- 密钥仅存本机并用 Windows DPAPI 加密；请勿在 Issue 或 PR 中粘贴任何真实密钥。
- 已报告并确认的漏洞会在修复后公开披露。
