<script setup lang="ts">
import { ShieldCheck } from "lucide-vue-next";

const emit = defineEmits<{ agree: []; refuse: [] }>();
</script>

<template>
  <div class="consent-gate" role="dialog" aria-modal="true" aria-labelledby="consent-title">
    <section class="consent-card">
      <span class="consent-badge"><ShieldCheck :size="26" /></span>
      <h1 id="consent-title">使用前请阅读</h1>
      <p class="consent-lead">
        PaySubmit（支付提交器）是<strong>开源</strong>的纯本地支付接口提交工具，源代码以 Apache-2.0
        协议公开，任何人都可以审计。
      </p>
      <ul class="consent-points">
        <li>不设任何服务端，<strong>不收集、不上传、不窃取</strong>任何隐私信息与密钥。</li>
        <li>
          无遥测、无埋点、不采集设备指纹；商编与密钥仅加密保存在本机（Windows
          DPAPI），界面不回显明文。
        </li>
        <li>
          软件只按你填写的内容向你指定的接口原样提交，<strong>提交请求可能产生真实资金变动</strong>，相关结果与责任由你自行承担。
        </li>
        <li>本项目与任何支付机构无隶属、代言或合作关系，不提供支付服务。</li>
      </ul>
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
.consent-actions {
  display: flex;
  justify-content: flex-end;
  gap: 10px;
  margin-top: 24px;
}
</style>
