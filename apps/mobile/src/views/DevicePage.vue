<script setup lang="ts">
import AppIcon from "../components/AppIcon.vue";
type Device = { name: string; address: string };
defineProps<{
  devices: Device[];
  connected: string;
  busy: boolean;
  connectionStatus: string;
  native: boolean;
}>();
const address = defineModel<string>("address", { required: true });
const code = defineModel<string>("code", { required: true });
const name = defineModel<string>("name", { required: true });
defineEmits<{ connect: [device?: Device]; forget: [device: Device] }>();
</script>
<template>
  <h2 v-if="devices.length" class="subheading">已保存的电脑</h2>
  <div v-if="devices.length" class="list-panel device-list">
    <div v-for="device in devices" :key="device.address" class="saved-device">
      <button
        class="list-row"
        :disabled="busy"
        :aria-pressed="connected === device.address"
        @click="$emit('connect', device)"
      >
        <span class="icon-disc secondary"><AppIcon name="computer" /></span
        ><span class="row-copy"
          ><strong>{{ device.name }}</strong
          ><small>{{ device.address }}</small></span
        ><AppIcon class="chevron" name="chevron" /></button
      ><button
        class="text-button remove-device"
        :disabled="busy"
        @click="$emit('forget', device)"
      >
        从本机移除
      </button>
    </div>
  </div>
  <h2 class="subheading">连接电脑</h2>
  <form class="panel form-panel" @submit.prevent="$emit('connect')">
    <label
      >电脑 IP 或地址<input
        v-model="address"
        placeholder="192.168.1.20:7337"
        required
        autocomplete="off"
        autocapitalize="off"
        spellcheck="false"
        inputmode="url" /></label
    ><label
      >手机名称<input
        v-model="name"
        maxlength="64"
        required
        autocomplete="nickname" /></label
    ><label
      >电脑配对码<input
        v-model="code"
        placeholder="电脑显示的配对码"
        type="password"
        autocomplete="off"
        autocapitalize="off"
        spellcheck="false" /></label
    ><button class="primary-button full-width" :disabled="busy">
      {{ connectionStatus === "connecting" ? "正在连接…" : "配对并连接" }}
    </button>
  </form>
  <p class="hint page-note">
    {{
      native
        ? "配对凭据安全保存在手机上，点击已保存的电脑即可重连。"
        : "浏览器仅在本次会话保存凭据；安装版可保存配对。"
    }}请在可信局域网或 VPN 内连接。
  </p>
</template>
