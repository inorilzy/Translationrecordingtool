<script setup lang="ts">
import { computed, nextTick, onMounted, onUnmounted, ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { emit, listen } from '@tauri-apps/api/event'

type Point = {
  x: number
  y: number
}

type Rect = {
  left: number
  top: number
  width: number
  height: number
}

type InteractionMode = 'idle' | 'drawing' | 'moving' | 'resizing'
type ResizeHandle = 'n' | 'ne' | 'e' | 'se' | 's' | 'sw' | 'w' | 'nw'

type ScreenPreview = {
  x: number
  y: number
  width: number
  height: number
  image: string
}

type SelectionStartPayload = {
  x: number
  y: number
  width: number
  height: number
  screens: ScreenPreview[]
}

const interactionMode = ref<InteractionMode>('idle')
const pointerStart = ref<Point | null>(null)
const interactionStartRect = ref<Rect | null>(null)
const activeResizeHandle = ref<ResizeHandle | null>(null)
const selectionRect = ref<Rect | null>(null)
const desktopBounds = ref({ x: 0, y: 0 })
const desktopSize = ref({ width: 1, height: 1 })
const screenPreviews = ref<ScreenPreview[]>([])
let unlistenReload: (() => void) | null = null

const minCssSelectionSize = 6
const resizeHandles: ResizeHandle[] = ['nw', 'n', 'ne', 'e', 'se', 's', 'sw', 'w']

const selectionStyle = computed(() => {
  const rect = selectionRect.value

  if (!rect) {
    return { display: 'none' }
  }

  return {
    left: `${rect.left}px`,
    top: `${rect.top}px`,
    width: `${rect.width}px`,
    height: `${rect.height}px`,
  }
})

const hintText = computed(() => {
  if (!selectionRect.value) {
    return '拖拽选择要 OCR 翻译的区域，按 Esc 取消'
  }
  return '拖动边框调整区域，按 Enter 确认，Esc 取消'
})

const selectionSizeText = computed(() => {
  const rect = selectionRect.value
  if (!rect) return ''

  const desktopRect = desktopRectFromCssRect(rect)
  return `${desktopRect.width} x ${desktopRect.height}`
})

const toolbarAbove = computed(() => {
  const rect = selectionRect.value
  if (!rect) return false
  return rect.top + rect.height + 46 > window.innerHeight
})

const viewportScale = computed(() => ({
  x: window.innerWidth / Math.max(desktopSize.value.width, 1),
  y: window.innerHeight / Math.max(desktopSize.value.height, 1),
}))

const previewStyles = computed(() => screenPreviews.value.map((screen) => {
  const scale = viewportScale.value

  return {
    screen,
    style: {
      left: `${(screen.x - desktopBounds.value.x) * scale.x}px`,
      top: `${(screen.y - desktopBounds.value.y) * scale.y}px`,
      width: `${screen.width * scale.x}px`,
      height: `${screen.height * scale.y}px`,
    },
  }
}))

function resetSelection() {
  interactionMode.value = 'idle'
  pointerStart.value = null
  interactionStartRect.value = null
  activeResizeHandle.value = null
  selectionRect.value = null
}

function cssPoint(event: PointerEvent): Point {
  return {
    x: event.clientX,
    y: event.clientY,
  }
}

function clampPoint(point: Point): Point {
  return {
    x: Math.min(Math.max(point.x, 0), window.innerWidth),
    y: Math.min(Math.max(point.y, 0), window.innerHeight),
  }
}

function rectFromPoints(start: Point, end: Point): Rect {
  return {
    left: Math.min(start.x, end.x),
    top: Math.min(start.y, end.y),
    width: Math.abs(end.x - start.x),
    height: Math.abs(end.y - start.y),
  }
}

function clampRect(rect: Rect): Rect {
  const width = Math.min(Math.max(rect.width, minCssSelectionSize), window.innerWidth)
  const height = Math.min(Math.max(rect.height, minCssSelectionSize), window.innerHeight)

  return {
    left: Math.min(Math.max(rect.left, 0), Math.max(window.innerWidth - width, 0)),
    top: Math.min(Math.max(rect.top, 0), Math.max(window.innerHeight - height, 0)),
    width,
    height,
  }
}

function resizeRect(startRect: Rect, start: Point, current: Point, handle: ResizeHandle): Rect {
  const dx = current.x - start.x
  const dy = current.y - start.y
  let { left, top, width, height } = startRect

  if (handle.includes('w')) {
    left += dx
    width -= dx
  }
  if (handle.includes('e')) {
    width += dx
  }
  if (handle.includes('n')) {
    top += dy
    height -= dy
  }
  if (handle.includes('s')) {
    height += dy
  }

  if (width < minCssSelectionSize) {
    if (handle.includes('w')) {
      left = startRect.left + startRect.width - minCssSelectionSize
    }
    width = minCssSelectionSize
  }

  if (height < minCssSelectionSize) {
    if (handle.includes('n')) {
      top = startRect.top + startRect.height - minCssSelectionSize
    }
    height = minCssSelectionSize
  }

  return clampRect({ left, top, width, height })
}

function desktopRectFromCssRect(rect: Rect) {
  const scale = viewportScale.value

  return {
    x: Math.round(desktopBounds.value.x + rect.left / scale.x),
    y: Math.round(desktopBounds.value.y + rect.top / scale.y),
    width: Math.round(rect.width / scale.x),
    height: Math.round(rect.height / scale.y),
  }
}

function onOverlayPointerDown(event: PointerEvent) {
  if (event.button !== 0) return

  const point = clampPoint(cssPoint(event))
  interactionMode.value = 'drawing'
  pointerStart.value = point
  interactionStartRect.value = null
  activeResizeHandle.value = null
  selectionRect.value = {
    left: point.x,
    top: point.y,
    width: 0,
    height: 0,
  }
  ;(event.currentTarget as HTMLElement).setPointerCapture(event.pointerId)
}

function onSelectionPointerDown(event: PointerEvent) {
  if (event.button !== 0 || !selectionRect.value) return

  interactionMode.value = 'moving'
  pointerStart.value = clampPoint(cssPoint(event))
  interactionStartRect.value = { ...selectionRect.value }
  activeResizeHandle.value = null
  ;(event.currentTarget as HTMLElement).setPointerCapture(event.pointerId)
}

function onResizeHandlePointerDown(handle: ResizeHandle, event: PointerEvent) {
  if (event.button !== 0 || !selectionRect.value) return

  interactionMode.value = 'resizing'
  pointerStart.value = clampPoint(cssPoint(event))
  interactionStartRect.value = { ...selectionRect.value }
  activeResizeHandle.value = handle
  ;(event.currentTarget as HTMLElement).setPointerCapture(event.pointerId)
}

function onPointerMove(event: PointerEvent) {
  const start = pointerStart.value
  const current = clampPoint(cssPoint(event))
  const startRect = interactionStartRect.value

  if (!start || interactionMode.value === 'idle') return

  if (interactionMode.value === 'drawing') {
    selectionRect.value = rectFromPoints(start, current)
    return
  }

  if (interactionMode.value === 'moving' && startRect) {
    selectionRect.value = clampRect({
      ...startRect,
      left: startRect.left + current.x - start.x,
      top: startRect.top + current.y - start.y,
    })
    return
  }

  if (interactionMode.value === 'resizing' && startRect && activeResizeHandle.value) {
    selectionRect.value = resizeRect(startRect, start, current, activeResizeHandle.value)
  }
}

function onPointerUp() {
  const rect = selectionRect.value

  if (interactionMode.value === 'drawing' && (!rect || rect.width < minCssSelectionSize || rect.height < minCssSelectionSize)) {
    resetSelection()
    return
  }

  interactionMode.value = 'idle'
  pointerStart.value = null
  interactionStartRect.value = null
  activeResizeHandle.value = null
}

async function completeSelection() {
  const rect = selectionRect.value
  if (!rect) return

  const desktopRect = desktopRectFromCssRect(rect)
  if (desktopRect.width < 4 || desktopRect.height < 4) {
    resetSelection()
    return
  }

  await emit('screenshot-selection-completed', {
    x: desktopRect.x,
    y: desktopRect.y,
    width: desktopRect.width,
    height: desktopRect.height,
  })
  resetSelection()
}

async function cancelSelection() {
  resetSelection()
  await emit('screenshot-selection-cancelled')
}

function preloadImage(src: string) {
  return new Promise<void>((resolve, reject) => {
    const image = new Image()
    image.onload = () => resolve()
    image.onerror = () => reject(new Error('截图预览加载失败'))
    image.src = src
  })
}

async function loadSelectionPayload() {
  screenPreviews.value = []
  const payload = await invoke<SelectionStartPayload>('get_screenshot_selection_payload')
  await Promise.all(payload.screens.map((screen) => preloadImage(screen.image)))
  desktopBounds.value = {
    x: payload.x,
    y: payload.y,
  }
  desktopSize.value = {
    width: payload.width,
    height: payload.height,
  }
  screenPreviews.value = payload.screens
  resetSelection()
  await nextTick()
}

async function prepareSelectionWindow() {
  try {
    await loadSelectionPayload()
    await emit('screenshot-selection-ready')
  } catch (error) {
    console.error('加载截图预览失败:', error)
    await emit('screenshot-selection-cancelled')
  }
}

function onKeyDown(event: KeyboardEvent) {
  if (event.key === 'Escape') {
    void cancelSelection()
  }
  if (event.key === 'Enter' && selectionRect.value) {
    void completeSelection()
  }
}

onMounted(async () => {
  document.addEventListener('keydown', onKeyDown)
  unlistenReload = await listen('screenshot-selection-reload', () => {
    void prepareSelectionWindow()
  })
  await prepareSelectionWindow()
})

onUnmounted(() => {
  document.removeEventListener('keydown', onKeyDown)
  unlistenReload?.()
})
</script>

<template>
  <main
    class="screenshot-overlay"
    @pointerdown="onOverlayPointerDown"
    @pointermove="onPointerMove"
    @pointerup="onPointerUp"
    @pointercancel="cancelSelection"
    @contextmenu.prevent="cancelSelection"
  >
    <div class="desktop-preview" aria-hidden="true">
      <img
        v-for="{ screen, style } in previewStyles"
        :key="`${screen.x}:${screen.y}:${screen.width}:${screen.height}`"
        class="screen-preview"
        :src="screen.image"
        :style="style"
        alt=""
        draggable="false"
      />
    </div>
    <div class="screen-dim" aria-hidden="true"></div>
    <div class="hint">{{ hintText }}</div>
    <div
      v-if="selectionRect"
      class="selection-box"
      :class="{ resizing: interactionMode === 'resizing', moving: interactionMode === 'moving' }"
      :style="selectionStyle"
      @pointerdown.stop="onSelectionPointerDown"
      @dblclick.stop="completeSelection"
    >
      <button
        v-for="handle in resizeHandles"
        :key="handle"
        class="resize-handle"
        :class="`handle-${handle}`"
        :aria-label="`调整${handle}方向`"
        type="button"
        @pointerdown.stop="onResizeHandlePointerDown(handle, $event)"
      ></button>
      <div class="selection-size">{{ selectionSizeText }}</div>
      <div
        class="selection-toolbar"
        :class="{ above: toolbarAbove }"
        @pointerdown.stop
      >
        <button type="button" class="toolbar-button secondary" @click.stop="cancelSelection">取消</button>
        <button type="button" class="toolbar-button primary" @click.stop="completeSelection">确认</button>
      </div>
    </div>
  </main>
</template>

<style scoped>
.screenshot-overlay {
  position: fixed;
  inset: 0;
  cursor: crosshair;
  background: #111827;
  user-select: none;
  overflow: hidden;
}

.desktop-preview,
.screen-dim {
  position: fixed;
  inset: 0;
  pointer-events: none;
}

.screen-preview {
  position: fixed;
  display: block;
  object-fit: fill;
  pointer-events: none;
  user-select: none;
}

.screen-dim {
  background: rgba(0, 0, 0, 0.24);
}

.hint {
  position: fixed;
  top: 18px;
  left: 50%;
  transform: translateX(-50%);
  padding: 8px 14px;
  background: rgba(20, 24, 31, 0.92);
  color: #fff;
  border: 1px solid rgba(255, 255, 255, 0.18);
  border-radius: 6px;
  font-size: 14px;
  line-height: 1.4;
  pointer-events: none;
  box-shadow: 0 8px 24px rgba(0, 0, 0, 0.3);
}

.selection-box {
  position: fixed;
  box-sizing: border-box;
  border: 1px solid #1f8fff;
  background: rgba(31, 143, 255, 0.08);
  box-shadow: 0 0 0 9999px rgba(0, 0, 0, 0.34);
  cursor: move;
  pointer-events: auto;
}

.selection-box.moving,
.selection-box.resizing {
  transition: none;
}

.selection-size {
  position: absolute;
  left: 0;
  top: -28px;
  min-width: 80px;
  padding: 4px 8px;
  background: rgba(20, 24, 31, 0.94);
  color: #fff;
  border-radius: 4px;
  font-size: 12px;
  line-height: 1.25;
  pointer-events: none;
  white-space: nowrap;
}

.selection-toolbar {
  position: absolute;
  right: 0;
  top: calc(100% + 8px);
  display: flex;
  gap: 8px;
  padding: 6px;
  background: rgba(20, 24, 31, 0.94);
  border: 1px solid rgba(255, 255, 255, 0.14);
  border-radius: 6px;
  box-shadow: 0 10px 24px rgba(0, 0, 0, 0.34);
}

.selection-toolbar.above {
  top: auto;
  bottom: calc(100% + 8px);
}

.toolbar-button {
  min-width: 54px;
  height: 28px;
  padding: 0 12px;
  border: 0;
  border-radius: 4px;
  color: #fff;
  font-size: 13px;
  line-height: 28px;
  cursor: pointer;
}

.toolbar-button.primary {
  background: #1f8fff;
}

.toolbar-button.secondary {
  background: rgba(255, 255, 255, 0.16);
}

.toolbar-button:hover {
  filter: brightness(1.08);
}

.resize-handle {
  position: absolute;
  width: 8px;
  height: 8px;
  padding: 0;
  border: 1px solid #1f8fff;
  border-radius: 50%;
  background: #fff;
}

.handle-nw {
  left: -5px;
  top: -5px;
  cursor: nwse-resize;
}

.handle-n {
  left: 50%;
  top: -5px;
  transform: translateX(-50%);
  cursor: ns-resize;
}

.handle-ne {
  right: -5px;
  top: -5px;
  cursor: nesw-resize;
}

.handle-e {
  right: -5px;
  top: 50%;
  transform: translateY(-50%);
  cursor: ew-resize;
}

.handle-se {
  right: -5px;
  bottom: -5px;
  cursor: nwse-resize;
}

.handle-s {
  left: 50%;
  bottom: -5px;
  transform: translateX(-50%);
  cursor: ns-resize;
}

.handle-sw {
  left: -5px;
  bottom: -5px;
  cursor: nesw-resize;
}

.handle-w {
  left: -5px;
  top: 50%;
  transform: translateY(-50%);
  cursor: ew-resize;
}
</style>
