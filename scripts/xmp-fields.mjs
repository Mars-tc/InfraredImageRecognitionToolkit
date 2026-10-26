// 提取 XMP 中的 DJI 字段（机型/Make/GPS）
import { readFileSync } from 'node:fs'
const s = readFileSync(process.argv[2]).toString('latin1')
for (const key of ['tiff:Make', 'tiff:Model', 'drone-dji:GpsLatitude', 'drone-dji:GpsLongitude', 'drone-dji:DroneModel', 'xmp:CreateDate']) {
  const m = s.match(new RegExp(key + '="([^"]*)"'))
  console.log(`${key} = ${m ? m[1] : '(none)'}`)
}
