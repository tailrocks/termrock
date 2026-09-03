import { expect, test } from '@playwright/test'

test('painter uses only the canonical Rust cursor position and visibility', async ({
  page,
}) => {
  await page.goto('/docs/components/text-input')
  const pixels = await page.evaluate(async () => {
    const { paintCanvas } = await import('/src/components/preview/painter.ts')
    const cell = {
      ch: ' ',
      fg: [255, 255, 255] as [number, number, number],
      bg: [0, 0, 0] as [number, number, number],
    }
    const frame = {
      story_id: 'cursor-test',
      title: 'Cursor test',
      component: 'Test',
      cols: 2,
      rows: 1,
      story_cols: 2,
      story_rows: 1,
      cells: [cell, cell],
      cursor: [1, 0] as [number, number],
      cursor_visible: true,
      interactive: true,
      theme: 'junie',
    }
    const canvas = document.createElement('canvas')
    const context = canvas.getContext('2d')
    if (!context) throw new Error('canvas 2d context unavailable')
    paintCanvas(canvas, frame, 9, 18, 1)
    const cursorPixel = Array.from(context.getImageData(13, 9, 1, 1).data)
    const plainPixel = Array.from(context.getImageData(4, 9, 1, 1).data)
    paintCanvas(canvas, { ...frame, cursor_visible: false }, 9, 18, 1)
    const hiddenPixel = Array.from(context.getImageData(13, 9, 1, 1).data)
    return { cursorPixel, plainPixel, hiddenPixel }
  })

  expect(pixels.cursorPixel).toEqual([255, 255, 255, 255])
  expect(pixels.plainPixel).toEqual([0, 0, 0, 255])
  expect(pixels.hiddenPixel).toEqual([0, 0, 0, 255])
})
