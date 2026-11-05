<script setup lang="ts">
import { ShieldCheck } from "lucide-vue-next";

const emit = defineEmits<{ agree: []; refuse: [] }>();
</script>

<template>
  <div class="consent-gate" role="dialog" aria-modal="true" aria-labelledby="consent-title">
    <section class="consent-card">
      <span class="consent-badge"><ShieldCheck :size="26" /></span>
      <h1 id="consent-title">开源与隐私声明</h1>
      <p class="consent-lead">
        PaySubmit（支付提交工具）是一款以 <strong>Apache-2.0</strong>
        许可证开源发布的纯本地支付接口提交工具，全部源代码公开可查、可独立审计。
      </p>
      <p class="consent-repo">
        <span class="consent-repo-label">开源地址</span>
        <code>https://github.com/UncleBang0214/PaySubmit</code>
      </p>
      <ul class="consent-points">
        <li>
          本软件不设任何形式的服务端，不收集、不上传、不窃取用户的任何隐私信息、商编或密钥，亦不进行任何数据回传。
        </li>
        <li>
          本软件不含遥测、埋点与设备指纹采集。商编与签名密钥仅以 Windows DPAPI
          加密存储于本机，界面只显示配置状态，不回显明文。
        </li>
        <li>
          本软件仅依据用户填写的参数，将其原样提交至用户自行指定的接口。<strong>提交请求可能产生真实的资金变动</strong>，相关结果、金额与合规责任均由操作者本人承担。
        </li>
        <li>本软件与任何支付机构均无隶属、代言或合作关系，不提供任何支付服务。</li>
      </ul>
      <p class="consent-footnote">
        点击「同意并使用」即表示您已阅读、理解并同意上述声明；如不同意，请选择「拒绝并退出」。
      </p>
      <footer class="consent-actions">
        <button type="button" class="btn ghost" @click="emit('refuse')">拒绝并退出</button>
        <button type="button" class="btn primary" autofocus @click="emit('agree')">
          同意并使用
        </button>
      </footer>
    </section>
  </div>
</template>

<style scoped>
.consent-gate {
  position: fixed;
  z-index: 200;
  inset: 0;
  display: grid;
  place-items: center;
  padding: 24px;
  background:
    radial-gradient(
      120% 120% at 50% 0%,
      color-mix(in srgb, var(--accent) 6%, transparent),
      transparent 60%
    ),
    var(--canvas);
}
.consent-card {
  width: min(560px, 100%);
  border: 1px solid var(--border);
  border-radius: 14px;
  padding: 30px 30px 24px;
  background: var(--surface);
  box-shadow: var(--shadow-dialog);
}
.consent-badge {
  display: grid;
  width: 54px;
  height: 54px;
  place-items: center;
  border-radius: 14px;
  background: var(--accent-soft);
  color: var(--accent);
}
.consent-card h1 {
  margin: 16px 0 0;
  color: var(--text);
  font-size: 20px;
  letter-spacing: 0.2px;
}
.consent-lead {
  margin: 10px 0 0;
  color: var(--text-muted);
  font-size: 13.5px;
  line-height: 1.7;
}
.consent-repo {
  display: flex;
  align-items: center;
  gap: 10px;
  margin: 14px 0 0;
  padding: 9px 12px;
  border: 1px solid var(--border);
  border-radius: 8px;
  background: var(--surface-subtle);
}
.consent-repo-label {
  flex: 0 0 auto;
  color: var(--text-faint);
  font-size: 12px;
}
.consent-repo code {
  min-width: 0;
  overflow: hidden;
  color: var(--accent);
  font-family: var(--font-code);
  font-size: 12.5px;
  text-overflow: ellipsis;
  white-space: nowrap;
  user-select: all;
}
.consent-points {
  display: grid;
  gap: 10px;
  margin: 16px 0 0;
  padding: 0;
  list-style: none;
}
.consent-points li {
  position: relative;
  padding-left: 22px;
  color: var(--text-muted);
  font-size: 13px;
  line-height: 1.65;
}
.consent-points li::before {
  content: "";
  position: absolute;
  top: 7px;
  left: 4px;
  width: 8px;
  height: 8px;
  border-radius: 50%;
  background: var(--accent);
  box-shadow: 0 0 0 3px var(--accent-soft);
}
.consent-points strong {
  color: var(--text);
  font-weight: 620;
}
.consent-footnote {
  margin: 16px 0 0;
  color: var(--text-faint);
  font-size: 12px;
  line-height: 1.6;
}
.consent-actions {
  display: flex;
  justify-content: flex-end;
  gap: 10px;
  margin-top: 20px;
}
</style>
