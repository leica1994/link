# Project UI Defaults

This project is a Tauri + Vue 3 + TypeScript app. Follow these defaults when adding or changing UI.

## General

- Prefer existing component and style patterns in `src/views/Settings.vue` and `src/styles.css`.
- Keep UI copy in Chinese unless the value is a product/API name such as `OpenAI`.
- Use Vue `<script setup lang="ts">` and typed enum-like values for fixed option sets.
- After UI changes, run `npm run build`.

## Visual Style

- Use the existing CSS variables for theme colors, surfaces, text, borders, and accents.
- Light theme panels and dialogs should use the existing warm beige surface palette:
  - panel/dialog background: `var(--bg-surface)` or `var(--dialog-bg)`
  - hover surface: `var(--bg-surface-hover)` or `var(--dialog-option-hover)`
- Dark theme must be supported for every new control through existing theme variables.
- New settings sections should use the existing structure:
  - `.settings-section`
  - `.section-heading`
  - `.settings-panel`
  - `.setting-row`
  - `.setting-icon`
  - `.setting-copy`
  - `.setting-title`
  - `.setting-subtitle`
- Keep settings rows dense and consistent. Do not introduce card-in-card layouts.

## Controls

- Do not use native `<select>` dropdowns for settings options.
- For enum choices, use a clickable `.setting-row-button` that shows the current value on the right, then open a modal dialog styled like the existing settings dialogs.
- Dialog option rows should use the existing single-choice radio pattern:
  - `.dialog-backdrop`
  - `.settings-dialog`
  - `.dialog-title`
  - `.dialog-options`
  - `.dialog-option`
  - `.dialog-radio`
- Dialogs must close on backdrop click and `Escape`.
- Right-side current values should use `.setting-inline-action` and `.setting-value`.
- Password/API key inputs must use the custom visibility button pattern with `Eye` / `EyeOff`; hide browser-native reveal controls.
- Toggles should follow the existing `.setting-toggle` pattern.
- Buttons for unimplemented actions should be disabled and visually consistent with `.settings-action`.

## Icons

- Use `lucide-vue-next` icons for UI icons.
- Do not hand-write SVG icons when a suitable Lucide icon exists.
- Match existing icon sizing and stroke widths, usually `:stroke-width="2.1"` for row icons and `2.4` for chevrons.

## Scrolling

- Settings pages may scroll internally, but scrollbars should stay hidden.
- Use the existing hidden scrollbar pattern on `.settings-page`.

## Accessibility

- Dialogs should use `role="dialog"`, `aria-modal="true"`, and a labelled heading.
- Dialog option groups should use `role="radiogroup"` and options should use `role="radio"` with `aria-checked`.
- Icon-only buttons need an `aria-label`.
- Decorative icons should use `aria-hidden="true"`.
