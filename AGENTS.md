# Project UI Defaults

This project is a Tauri + Vue 3 + TypeScript app. Follow these defaults when adding or changing UI.

## General

- Prefer existing component and style patterns in `src/views/Settings.vue` and `src/styles.css`.
- Keep UI copy in Chinese unless the value is a product/API name such as `OpenAI`.
- Use Vue `<script setup lang="ts">` and typed enum-like values for fixed option sets.
- After UI changes, run `npm run build`.

## Settings Persistence

- Store settings in the local SQLite database managed by the Tauri backend, not ad hoc frontend-only state.
- Use `src-tauri/src/settings.rs` as the source of truth for the settings schema, defaults, load command, and save command.
- When adding or changing a setting, update both the Rust `AppSettings`/database mapping and the Vue `AppSettings` type/load-save snapshot.
- Keep LLM configuration as three independent service-specific records keyed by LLM service:
  - `OpenAI`
  - `OpenAI Responses`
  - `Anthropic`
- The fields under LLM configuration (`Base URL`, `API Key`, model, reasoning effort, streaming) must switch with the selected LLM service and must not be shared across services.

## AI Backend

- Keep AI API calls in the Tauri backend. Do not call LLM APIs directly from Vue components.
- Use `src-tauri/src/ai.rs` as the source of truth for shared LLM clients, connection checks, and AI request concurrency.
- Reuse the managed `AiService` for translation, optimization, subtitle correction, smart segmentation, and connection checks instead of creating ad hoc HTTP clients in feature commands.
- Keep the AI concurrency limit tied to `translation_thread_count`; saving settings must update this limit dynamically.
- `translation_batch_size` controls work chunking only and must not change the AI concurrency limit.
- AI network/provider retries must be centralized in `AiService`, with 3 total attempts by default. Retries must reacquire the AI concurrency permit so they queue behind the same thread pool limit.
- Normal LLM requests should not set a total request timeout because reasoning models may take longer than fixed client limits. Keep the shared connection timeout at 60 seconds, and keep LLM connection checks capped at 60 seconds.
- LLM response validation retries are separate from network/provider retries. If a request succeeds but the structured content is invalid, the feature logic may retry with feedback while still sending each retry through `AiService`.
- Connection checks should use the currently saved LLM settings, send a minimal non-streaming test request, and never expose API keys in responses or logs.

## Transcription AI Workflow

- Use `src-tauri/src/subtitle_ai.rs` for subtitle correction and smart segmentation behavior.
- Smart segmentation should batch by the internal word-count cap (`MAX_SPLIT_CHUNK_WORDS`) for better context, not by `translation_batch_size`.
- `translation_batch_size` should be used for subtitle correction chunking.
- Single short smart-segmentation blocks that already satisfy subtitle length rules should skip the LLM request and be marked as segmented, not as retained original text.
- Smart segmentation and subtitle correction should update the visible subtitle table while running. Only batches currently submitted for execution should show processing states such as `断句中` or `校正中`.
- The translate page should use one progress bar whose value represents the current active stage. Transcription, smart segmentation, and subtitle correction each maintain independent 0-100 stage progress behind that single bar.
- Keep `/translate` route state alive when switching app pages so an in-progress transcription view restores when returning to the Translate page.
- Logging for transcription and AI processing should stay concise. Log starts, completions, partial failures, and validation failure reasons; avoid logging full prompts or full LLM responses unless temporarily debugging a specific issue.
- App logs are global application logs, not translation-only logs. Store them directly in the app log directory with compact timestamp file names such as `20260608152815.log`; do not create date subdirectories or feature-specific file name prefixes. Open log directories through backend commands instead of frontend plugin path permissions.

## Visual Style

- Use the existing CSS variables for theme colors, surfaces, text, borders, and accents.
- Keep the app visual tone quiet, dense, and work-focused. Avoid marketing-style hero layouts, oversized decorative sections, floating page cards, nested cards, and purely decorative gradients.
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

## Page Layout

- Regular app pages should use the full-height `.page` shell and scroll internally when needed.
- Page horizontal padding should follow the existing responsive pattern: `24px clamp(22px, 3vw, 56px) 44px` on desktop and `22px 24px 44px` on narrow screens.
- Main page content should align to the same content track used by the translate page:
  - desktop width: `width: min(100%, 1440px)`
  - centered with `margin-left/right: auto`
  - narrow screens: `width: 100%`
- Top-level page titles must use `.page-title` and keep the existing `22px`, `750` weight, and `1.2` line-height.
- Pages with a top title only, such as Settings, should give the title row the same visual height as `.translate-header` so the title text aligns vertically with the Translate page title.
- The content below a top title/header should start at the same visual rhythm as Translate:
  - desktop gap: `44px`
  - narrow-screen gap: `34px`
- Do not introduce page-specific max widths that make Settings, Translate, or similar full-page workflows visually inconsistent. If a page needs a constrained layout, first check whether it should share the translate/settings `1440px` track.

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
