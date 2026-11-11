// 缺陷等级常量（与 Rust diagnosis.rs 保持一致）
export const DEFECT_LEVELS = [
  { value: 0, label: '其他', color: '#8B98A9' },
  { value: 1, label: '一般', color: '#F2B705' },
  { value: 2, label: '重大', color: '#F57C00' },
  { value: 3, label: '紧急', color: '#E02020' }
]

export const levelInfo = (level) =>
  Number.isInteger(level) && level >= 0 && level < DEFECT_LEVELS.length
    ? DEFECT_LEVELS[level]
    : DEFECT_LEVELS[0]
