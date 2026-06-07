import sharp from 'sharp'
import { mkdirSync } from 'fs'

mkdirSync('./src-tauri/icons', { recursive: true })
await sharp('C:/Users/leica/Desktop/1.svg')
  .resize(1024, 1024)
  .png()
  .toFile('./src-tauri/icons/app-icon.png')

console.log('Icon source generated: src-tauri/icons/app-icon.png')
