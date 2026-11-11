<script setup>
// F6：元数据面板（机型/时间/GPS/ε/距离/湿度/反射温/大气温/温度范围）
import { useImageStore } from '../stores/image.js'
import { fmtTemp, fmtEmissivity, fmtDistance, fmtHumidity } from '../utils/format.js'

const store = useImageStore()
</script>

<template>
  <section v-if="store.meta" class="panel-card meta-card">
    <h3 class="panel-title">元数据</h3>
    <div class="meta-grid">
      <span class="k">机型</span><span class="v">{{ store.meta.camera_make }} {{ store.meta.camera_model }}</span>
      <span class="k">拍摄时间</span><span class="v">{{ store.meta.datetime || '--' }}</span>
      <span class="k">GPS</span><span class="v">{{ store.meta.gps || '--' }}</span>
      <span class="k">分辨率</span><span class="v">{{ store.meta.width }}×{{ store.meta.height }}</span>
      <span class="k">发射率</span><span class="v">{{ fmtEmissivity(store.meta.emissivity) }}</span>
      <span class="k">距离</span><span class="v">{{ fmtDistance(store.meta.distance) }}</span>
      <span class="k">湿度</span><span class="v">{{ fmtHumidity(store.meta.humidity) }}</span>
      <span class="k">反射温度</span><span class="v">{{ fmtTemp(store.meta.reflected_temp) }}</span>
      <span class="k">大气温度</span><span class="v">{{ fmtTemp(store.meta.atmospheric_temp) }}</span>
      <span class="k">温度范围</span><span class="v hot">{{ fmtTemp(store.meta.temp_min) }} ~ {{ fmtTemp(store.meta.temp_max) }}</span>
    </div>
  </section>
</template>

<style scoped>
.panel-title {
  margin: 0 0 8px;
  font-size: 13px;
  font-weight: 600;
  color: var(--ip-text);
}

.meta-grid {
  display: grid;
  grid-template-columns: 64px 1fr;
  gap: 6px 10px;
  font-size: 12px;
}

.meta-grid .k {
  color: var(--ip-text-muted);
}

.meta-grid .v {
  color: var(--ip-text);
  font-variant-numeric: tabular-nums;
  word-break: break-all;
}

.meta-grid .v.hot {
  color: var(--ip-danger);
  font-weight: 600;
}
</style>
