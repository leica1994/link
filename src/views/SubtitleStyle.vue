<template>
  <div class="page translate-page">
    <header class="translate-header">
      <h1 class="page-title">字幕样式</h1>
    </header>

    <main class="translate-workspace subtitle-style-workspace">
      <div class="translate-grid">
        <!-- 左侧：样式列表 -->
        <section class="settings-section" aria-labelledby="style-list-title">
          <div id="style-list-title" class="section-heading">
            <List aria-hidden="true" />
            <span>样式预设</span>
          </div>

          <div class="settings-panel">
            <div class="subtitle-style-list">
              <button
                v-for="style in styles"
                :key="style.id"
                class="subtitle-style-item"
                :class="{ active: currentStyle?.id === style.id }"
                type="button"
                @click="selectStyle(style)"
              >
                <div class="subtitle-style-item-content">
                  <Palette class="subtitle-style-item-icon" :stroke-width="2.1" aria-hidden="true" />
                  <div class="subtitle-style-item-info">
                    <span class="subtitle-style-item-name">{{ style.name }}</span>
                    <span v-if="style.isDefault" class="subtitle-style-item-badge">默认</span>
                  </div>
                </div>
                <Check v-if="currentStyle?.id === style.id" class="subtitle-style-item-check" :stroke-width="2.4" aria-hidden="true" />
              </button>
            </div>

            <div class="subtitle-style-actions">
              <button class="settings-action" type="button" @click="createNewStyle">
                <Plus :stroke-width="2.1" aria-hidden="true" />
                <span>新建样式</span>
              </button>
              <button
                class="settings-action settings-action-danger"
                type="button"
                :disabled="!currentStyle || currentStyle.isDefault"
                @click="confirmDeleteStyle"
              >
                <Trash2 :stroke-width="2.1" aria-hidden="true" />
                <span>删除样式</span>
              </button>
            </div>
          </div>
        </section>

        <!-- 右侧：主字幕样式 -->
        <section class="settings-section" aria-labelledby="primary-style-title">
          <div id="primary-style-title" class="section-heading">
            <Type aria-hidden="true" />
            <span>主字幕样式</span>
          </div>

          <div class="settings-panel">
            <div class="setting-row">
              <FontIcon class="setting-icon" :stroke-width="2.1" aria-hidden="true" />
              <div class="setting-copy">
                <div class="setting-title">字体</div>
                <div class="setting-subtitle">主字幕的字体</div>
              </div>
              <input
                v-model="primaryFontName"
                class="settings-input setting-inline-input"
                type="text"
                placeholder="Arial"
                @change="saveCurrentStyle"
              />
            </div>

            <div class="setting-row">
              <Maximize class="setting-icon" :stroke-width="2.1" aria-hidden="true" />
              <div class="setting-copy">
                <div class="setting-title">字号</div>
                <div class="setting-subtitle">主字幕的大小</div>
              </div>
              <input
                v-model.number="primaryFontSize"
                class="settings-input setting-inline-input-small"
                type="number"
                min="8"
                max="200"
                @change="saveCurrentStyle"
              />
            </div>

            <div class="setting-row">
              <Palette class="setting-icon" :stroke-width="2.1" aria-hidden="true" />
              <div class="setting-copy">
                <div class="setting-title">文字颜色</div>
                <div class="setting-subtitle">主字幕的文字颜色</div>
              </div>
              <input
                v-model="primaryColor"
                class="settings-input setting-inline-input-small"
                type="color"
                @change="saveCurrentStyle"
              />
            </div>

            <div class="setting-row">
              <PenTool class="setting-icon" :stroke-width="2.1" aria-hidden="true" />
              <div class="setting-copy">
                <div class="setting-title">描边颜色</div>
                <div class="setting-subtitle">主字幕的边框颜色</div>
              </div>
              <input
                v-model="primaryOutlineColor"
                class="settings-input setting-inline-input-small"
                type="color"
                @change="saveCurrentStyle"
              />
            </div>

            <div class="setting-row">
              <Circle class="setting-icon" :stroke-width="2.1" aria-hidden="true" />
              <div class="setting-copy">
                <div class="setting-title">描边宽度</div>
                <div class="setting-subtitle">主字幕的边框粗细</div>
              </div>
              <input
                v-model.number="primaryOutlineWidth"
                class="settings-input setting-inline-input-small"
                type="number"
                min="0"
                max="10"
                step="0.5"
                @change="saveCurrentStyle"
              />
            </div>

            <div class="setting-row">
              <Space class="setting-icon" :stroke-width="2.1" aria-hidden="true" />
              <div class="setting-copy">
                <div class="setting-title">字符间距</div>
                <div class="setting-subtitle">主字幕的字符间距</div>
              </div>
              <input
                v-model.number="primarySpacing"
                class="settings-input setting-inline-input-small"
                type="number"
                min="0"
                max="10"
                step="0.5"
                @change="saveCurrentStyle"
              />
            </div>
          </div>
        </section>

        <!-- 下方：副字幕样式 -->
        <section class="settings-section" aria-labelledby="secondary-style-title">
          <div id="secondary-style-title" class="section-heading">
            <Type aria-hidden="true" />
            <span>副字幕样式</span>
          </div>

          <div class="settings-panel">
            <div class="setting-row">
              <FontIcon class="setting-icon" :stroke-width="2.1" aria-hidden="true" />
              <div class="setting-copy">
                <div class="setting-title">字体</div>
                <div class="setting-subtitle">副字幕的字体</div>
              </div>
              <input
                v-model="secondaryFontName"
                class="settings-input setting-inline-input"
                type="text"
                placeholder="Arial"
                @change="saveCurrentStyle"
              />
            </div>

            <div class="setting-row">
              <Maximize class="setting-icon" :stroke-width="2.1" aria-hidden="true" />
              <div class="setting-copy">
                <div class="setting-title">字号</div>
                <div class="setting-subtitle">副字幕的大小</div>
              </div>
              <input
                v-model.number="secondaryFontSize"
                class="settings-input setting-inline-input-small"
                type="number"
                min="8"
                max="200"
                @change="saveCurrentStyle"
              />
            </div>

            <div class="setting-row">
              <Palette class="setting-icon" :stroke-width="2.1" aria-hidden="true" />
              <div class="setting-copy">
                <div class="setting-title">文字颜色</div>
                <div class="setting-subtitle">副字幕的文字颜色</div>
              </div>
              <input
                v-model="secondaryColor"
                class="settings-input setting-inline-input-small"
                type="color"
                @change="saveCurrentStyle"
              />
            </div>

            <div class="setting-row">
              <PenTool class="setting-icon" :stroke-width="2.1" aria-hidden="true" />
              <div class="setting-copy">
                <div class="setting-title">描边颜色</div>
                <div class="setting-subtitle">副字幕的边框颜色</div>
              </div>
              <input
                v-model="secondaryOutlineColor"
                class="settings-input setting-inline-input-small"
                type="color"
                @change="saveCurrentStyle"
              />
            </div>

            <div class="setting-row">
              <Circle class="setting-icon" :stroke-width="2.1" aria-hidden="true" />
              <div class="setting-copy">
                <div class="setting-title">描边宽度</div>
                <div class="setting-subtitle">副字幕的边框粗细</div>
              </div>
              <input
                v-model.number="secondaryOutlineWidth"
                class="settings-input setting-inline-input-small"
                type="number"
                min="0"
                max="10"
                step="0.5"
                @change="saveCurrentStyle"
              />
            </div>

            <div class="setting-row">
              <Space class="setting-icon" :stroke-width="2.1" aria-hidden="true" />
              <div class="setting-copy">
                <div class="setting-title">字符间距</div>
                <div class="setting-subtitle">副字幕的字符间距</div>
              </div>
              <input
                v-model.number="secondarySpacing"
                class="settings-input setting-inline-input-small"
                type="number"
                min="0"
                max="10"
                step="0.5"
                @change="saveCurrentStyle"
              />
            </div>
          </div>
        </section>

        <!-- 通用设置 -->
        <section class="settings-section" aria-labelledby="general-style-title">
          <div id="general-style-title" class="section-heading">
            <SlidersHorizontal aria-hidden="true" />
            <span>通用设置</span>
          </div>

          <div class="settings-panel">
            <div class="setting-row">
              <MoveVertical class="setting-icon" :stroke-width="2.1" aria-hidden="true" />
              <div class="setting-copy">
                <div class="setting-title">垂直间距</div>
                <div class="setting-subtitle">主副字幕之间的间距</div>
              </div>
              <input
                v-model.number="verticalSpacing"
                class="settings-input setting-inline-input-small"
                type="number"
                min="0"
                max="100"
                @change="saveCurrentStyle"
              />
            </div>
          </div>
        </section>
      </div>
    </main>

    <!-- 新建样式对话框 -->
    <Teleport to="body">
      <div v-if="showCreateDialog" class="dialog-backdrop" role="presentation" @click.self="closeCreateDialog">
        <section class="settings-dialog" role="dialog" aria-modal="true" aria-labelledby="create-style-dialog-title">
          <h2 id="create-style-dialog-title" class="dialog-title">新建样式</h2>
          <div class="dialog-content">
            <label class="settings-field">
              <span class="settings-field-label">样式名称</span>
              <input
                v-model="newStyleName"
                class="settings-input"
                type="text"
                placeholder="输入样式名称"
                autofocus
                @keyup.enter="confirmCreateStyle"
              />
            </label>
          </div>
          <div class="dialog-actions">
            <button class="settings-action" type="button" @click="closeCreateDialog">取消</button>
            <button class="settings-action" type="button" :disabled="!newStyleName.trim()" @click="confirmCreateStyle">
              创建
            </button>
          </div>
        </section>
      </div>
    </Teleport>

    <!-- 删除确认对话框 -->
    <Teleport to="body">
      <div v-if="showDeleteDialog" class="dialog-backdrop" role="presentation" @click.self="closeDeleteDialog">
        <section class="settings-dialog" role="dialog" aria-modal="true" aria-labelledby="delete-style-dialog-title">
          <h2 id="delete-style-dialog-title" class="dialog-title">删除样式</h2>
          <div class="dialog-content">
            <p>确定要删除样式「{{ currentStyle?.name }}」吗？此操作无法撤销。</p>
          </div>
          <div class="dialog-actions">
            <button class="settings-action" type="button" @click="closeDeleteDialog">取消</button>
            <button class="settings-action settings-action-danger" type="button" @click="confirmDelete">删除</button>
          </div>
        </section>
      </div>
    </Teleport>
  </div>
</template>

<script setup lang="ts">
import { invoke } from '@tauri-apps/api/core'
import { onMounted, ref } from 'vue'
import {
  Check,
  Circle,
  List,
  Maximize,
  MoveVertical,
  Palette,
  PenTool,
  Plus,
  SlidersHorizontal,
  Space,
  Trash2,
  Type,
} from 'lucide-vue-next'

const FontIcon = Type

defineOptions({ name: 'SubtitleStyle' })

type SubtitleStyle = {
  id: string
  name: string
  isDefault: boolean
  primaryFontName: string
  primaryFontSize: number
  primaryColor: string
  primaryOutlineColor: string
  primaryOutlineWidth: number
  primarySpacing: number
  secondaryFontName: string
  secondaryFontSize: number
  secondaryColor: string
  secondaryOutlineColor: string
  secondaryOutlineWidth: number
  secondarySpacing: number
  verticalSpacing: number
  createdAt: string
  updatedAt: string
}

const styles = ref<SubtitleStyle[]>([])
const currentStyle = ref<SubtitleStyle | null>(null)

const primaryFontName = ref('Arial')
const primaryFontSize = ref(48)
const primaryColor = ref('#FFFFFF')
const primaryOutlineColor = ref('#000000')
const primaryOutlineWidth = ref(2.0)
const primarySpacing = ref(0.0)

const secondaryFontName = ref('Arial')
const secondaryFontSize = ref(36)
const secondaryColor = ref('#FFFFFF')
const secondaryOutlineColor = ref('#000000')
const secondaryOutlineWidth = ref(2.0)
const secondarySpacing = ref(0.0)

const verticalSpacing = ref(15)

const showCreateDialog = ref(false)
const newStyleName = ref('')
const showDeleteDialog = ref(false)

const loadStyles = async () => {
  try {
    const result = await invoke<SubtitleStyle[]>('list_subtitle_styles')
    styles.value = result

    if (result.length > 0 && !currentStyle.value) {
      selectStyle(result[0])
    }
  } catch (error) {
    console.error('加载字幕样式失败', error)
  }
}

const selectStyle = (style: SubtitleStyle) => {
  currentStyle.value = style
  primaryFontName.value = style.primaryFontName
  primaryFontSize.value = style.primaryFontSize
  primaryColor.value = style.primaryColor
  primaryOutlineColor.value = style.primaryOutlineColor
  primaryOutlineWidth.value = style.primaryOutlineWidth
  primarySpacing.value = style.primarySpacing
  secondaryFontName.value = style.secondaryFontName
  secondaryFontSize.value = style.secondaryFontSize
  secondaryColor.value = style.secondaryColor
  secondaryOutlineColor.value = style.secondaryOutlineColor
  secondaryOutlineWidth.value = style.secondaryOutlineWidth
  secondarySpacing.value = style.secondarySpacing
  verticalSpacing.value = style.verticalSpacing
}

const saveCurrentStyle = async () => {
  if (!currentStyle.value) {
    return
  }

  try {
    const updated = await invoke<SubtitleStyle>('update_subtitle_style', {
      request: {
        id: currentStyle.value.id,
        name: currentStyle.value.name,
        primaryFontName: primaryFontName.value,
        primaryFontSize: primaryFontSize.value,
        primaryColor: primaryColor.value,
        primaryOutlineColor: primaryOutlineColor.value,
        primaryOutlineWidth: primaryOutlineWidth.value,
        primarySpacing: primarySpacing.value,
        secondaryFontName: secondaryFontName.value,
        secondaryFontSize: secondaryFontSize.value,
        secondaryColor: secondaryColor.value,
        secondaryOutlineColor: secondaryOutlineColor.value,
        secondaryOutlineWidth: secondaryOutlineWidth.value,
        secondarySpacing: secondarySpacing.value,
        verticalSpacing: verticalSpacing.value,
      },
    })

    currentStyle.value = updated
    const index = styles.value.findIndex((s) => s.id === updated.id)
    if (index >= 0) {
      styles.value[index] = updated
    }
  } catch (error) {
    console.error('保存字幕样式失败', error)
  }
}

const createNewStyle = () => {
  newStyleName.value = ''
  showCreateDialog.value = true
}

const closeCreateDialog = () => {
  showCreateDialog.value = false
  newStyleName.value = ''
}

const confirmCreateStyle = async () => {
  if (!newStyleName.value.trim()) {
    return
  }

  try {
    const created = await invoke<SubtitleStyle>('create_subtitle_style', {
      request: {
        name: newStyleName.value.trim(),
        primaryFontName: primaryFontName.value,
        primaryFontSize: primaryFontSize.value,
        primaryColor: primaryColor.value,
        primaryOutlineColor: primaryOutlineColor.value,
        primaryOutlineWidth: primaryOutlineWidth.value,
        primarySpacing: primarySpacing.value,
        secondaryFontName: secondaryFontName.value,
        secondaryFontSize: secondaryFontSize.value,
        secondaryColor: secondaryColor.value,
        secondaryOutlineColor: secondaryOutlineColor.value,
        secondaryOutlineWidth: secondaryOutlineWidth.value,
        secondarySpacing: secondarySpacing.value,
        verticalSpacing: verticalSpacing.value,
      },
    })

    styles.value.push(created)
    selectStyle(created)
    closeCreateDialog()
  } catch (error) {
    console.error('创建字幕样式失败', error)
    alert(`创建失败: ${error}`)
  }
}

const confirmDeleteStyle = () => {
  if (!currentStyle.value || currentStyle.value.isDefault) {
    return
  }
  showDeleteDialog.value = true
}

const closeDeleteDialog = () => {
  showDeleteDialog.value = false
}

const confirmDelete = async () => {
  if (!currentStyle.value) {
    return
  }

  try {
    await invoke('delete_subtitle_style', { id: currentStyle.value.id })
    const deletedId = currentStyle.value.id
    styles.value = styles.value.filter((s) => s.id !== deletedId)

    if (styles.value.length > 0) {
      selectStyle(styles.value[0])
    } else {
      currentStyle.value = null
    }

    closeDeleteDialog()
  } catch (error) {
    console.error('删除字幕样式失败', error)
    alert(`删除失败: ${error}`)
  }
}

onMounted(() => {
  void loadStyles()
})
</script>

<style scoped>
.subtitle-style-workspace {
  padding: 0;
}

.subtitle-style-list {
  display: flex;
  flex-direction: column;
  gap: 8px;
  margin-bottom: 16px;
}

.subtitle-style-item {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 12px 16px;
  background: transparent;
  border: 1px solid var(--border-color);
  border-radius: 8px;
  cursor: pointer;
  transition: all 0.2s;
}

.subtitle-style-item:hover {
  background: var(--hover-bg);
  border-color: var(--primary-color);
}

.subtitle-style-item.active {
  background: var(--primary-color-10);
  border-color: var(--primary-color);
}

.subtitle-style-item-content {
  display: flex;
  align-items: center;
  gap: 12px;
}

.subtitle-style-item-icon {
  flex-shrink: 0;
  width: 20px;
  height: 20px;
  color: var(--text-secondary);
}

.subtitle-style-item.active .subtitle-style-item-icon {
  color: var(--primary-color);
}

.subtitle-style-item-info {
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.subtitle-style-item-name {
  font-size: 14px;
  font-weight: 500;
  color: var(--text-primary);
}

.subtitle-style-item-badge {
  display: inline-block;
  padding: 2px 8px;
  font-size: 12px;
  color: var(--primary-color);
  background: var(--primary-color-10);
  border-radius: 4px;
  width: fit-content;
}

.subtitle-style-item-check {
  flex-shrink: 0;
  width: 20px;
  height: 20px;
  color: var(--primary-color);
}

.subtitle-style-actions {
  display: flex;
  gap: 8px;
}

.subtitle-style-actions .settings-action {
  flex: 1;
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 6px;
  padding: 8px 12px;
  font-size: 14px;
}

.subtitle-style-actions .settings-action svg {
  width: 16px;
  height: 16px;
}

.setting-inline-input,
.setting-inline-input-small {
  padding: 6px 12px;
  font-size: 14px;
  border: 1px solid var(--border-color);
  border-radius: 6px;
  background: var(--bg-secondary);
  color: var(--text-primary);
  outline: none;
  transition: all 0.2s;
}

.setting-inline-input {
  min-width: 180px;
}

.setting-inline-input-small {
  width: 80px;
  text-align: center;
}

.setting-inline-input:focus,
.setting-inline-input-small:focus {
  border-color: var(--primary-color);
}

.setting-inline-input[type='color'],
.setting-inline-input-small[type='color'] {
  width: 60px;
  height: 36px;
  padding: 4px;
  cursor: pointer;
}

.settings-action-danger {
  background: var(--danger-color);
  color: white;
}

.settings-action-danger:hover:not(:disabled) {
  background: var(--danger-color-dark);
}

.settings-action-danger:disabled {
  background: var(--bg-tertiary);
  color: var(--text-disabled);
  cursor: not-allowed;
}

.dialog-content p {
  margin: 0;
  color: var(--text-secondary);
  line-height: 1.6;
}

.settings-field {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.settings-field-label {
  font-size: 14px;
  font-weight: 500;
  color: var(--text-primary);
}
</style>
