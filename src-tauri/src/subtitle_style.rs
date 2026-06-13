use rusqlite::params;
use serde::{Deserialize, Serialize};

use crate::settings::SettingsStore;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubtitleStyle {
    pub id: String,
    pub name: String,
    pub is_default: bool,
    pub render_mode: String,
    pub subtitle_layout: String,
    pub preview_text_mode: String,
    pub primary_font_name: String,
    pub primary_font_size: u32,
    pub primary_color: String,
    pub primary_outline_color: String,
    pub primary_outline_width: f64,
    pub primary_spacing: f64,
    pub primary_margin_bottom: u32,
    pub secondary_font_name: String,
    pub secondary_font_size: u32,
    pub secondary_color: String,
    pub secondary_outline_color: String,
    pub secondary_outline_width: f64,
    pub secondary_spacing: f64,
    pub vertical_spacing: u32,
    pub rounded_font_name: String,
    pub rounded_font_size: u32,
    pub rounded_text_color: String,
    pub rounded_background_color: String,
    pub rounded_corner_radius: u32,
    pub rounded_padding_x: u32,
    pub rounded_padding_y: u32,
    pub rounded_margin_bottom: u32,
    pub rounded_line_spacing: u32,
    pub rounded_letter_spacing: u32,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateSubtitleStyleRequest {
    pub name: String,
    pub render_mode: String,
    pub subtitle_layout: String,
    pub preview_text_mode: String,
    pub primary_font_name: String,
    pub primary_font_size: u32,
    pub primary_color: String,
    pub primary_outline_color: String,
    pub primary_outline_width: f64,
    pub primary_spacing: f64,
    pub primary_margin_bottom: u32,
    pub secondary_font_name: String,
    pub secondary_font_size: u32,
    pub secondary_color: String,
    pub secondary_outline_color: String,
    pub secondary_outline_width: f64,
    pub secondary_spacing: f64,
    pub vertical_spacing: u32,
    pub rounded_font_name: String,
    pub rounded_font_size: u32,
    pub rounded_text_color: String,
    pub rounded_background_color: String,
    pub rounded_corner_radius: u32,
    pub rounded_padding_x: u32,
    pub rounded_padding_y: u32,
    pub rounded_margin_bottom: u32,
    pub rounded_line_spacing: u32,
    pub rounded_letter_spacing: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateSubtitleStyleRequest {
    pub id: String,
    pub name: String,
    pub render_mode: String,
    pub subtitle_layout: String,
    pub preview_text_mode: String,
    pub primary_font_name: String,
    pub primary_font_size: u32,
    pub primary_color: String,
    pub primary_outline_color: String,
    pub primary_outline_width: f64,
    pub primary_spacing: f64,
    pub primary_margin_bottom: u32,
    pub secondary_font_name: String,
    pub secondary_font_size: u32,
    pub secondary_color: String,
    pub secondary_outline_color: String,
    pub secondary_outline_width: f64,
    pub secondary_spacing: f64,
    pub vertical_spacing: u32,
    pub rounded_font_name: String,
    pub rounded_font_size: u32,
    pub rounded_text_color: String,
    pub rounded_background_color: String,
    pub rounded_corner_radius: u32,
    pub rounded_padding_x: u32,
    pub rounded_padding_y: u32,
    pub rounded_margin_bottom: u32,
    pub rounded_line_spacing: u32,
    pub rounded_letter_spacing: u32,
}

#[tauri::command]
pub fn list_subtitle_styles(
    store: tauri::State<'_, SettingsStore>,
) -> Result<Vec<SubtitleStyle>, String> {
    store.with_connection(|connection| {
        let mut statement = connection
            .prepare(
                "
                SELECT
                    id, name, is_default,
                    render_mode, subtitle_layout, preview_text_mode,
                    primary_font_name, primary_font_size, primary_color,
                    primary_outline_color, primary_outline_width, primary_spacing,
                    primary_margin_bottom,
                    secondary_font_name, secondary_font_size, secondary_color,
                    secondary_outline_color, secondary_outline_width, secondary_spacing,
                    vertical_spacing,
                    rounded_font_name, rounded_font_size, rounded_text_color,
                    rounded_background_color, rounded_corner_radius, rounded_padding_x,
                    rounded_padding_y, rounded_margin_bottom, rounded_line_spacing,
                    rounded_letter_spacing,
                    created_at, updated_at
                FROM subtitle_styles
                ORDER BY is_default DESC, name ASC
                ",
            )
            .map_err(|error| format!("无法查询字幕样式: {error}"))?;

        let styles = statement
            .query_map([], |row| {
                Ok(SubtitleStyle {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    is_default: row.get::<_, i64>(2)? != 0,
                    render_mode: row.get(3)?,
                    subtitle_layout: row.get(4)?,
                    preview_text_mode: row.get(5)?,
                    primary_font_name: row.get(6)?,
                    primary_font_size: row.get(7)?,
                    primary_color: row.get(8)?,
                    primary_outline_color: row.get(9)?,
                    primary_outline_width: row.get(10)?,
                    primary_spacing: row.get(11)?,
                    primary_margin_bottom: row.get(12)?,
                    secondary_font_name: row.get(13)?,
                    secondary_font_size: row.get(14)?,
                    secondary_color: row.get(15)?,
                    secondary_outline_color: row.get(16)?,
                    secondary_outline_width: row.get(17)?,
                    secondary_spacing: row.get(18)?,
                    vertical_spacing: row.get(19)?,
                    rounded_font_name: row.get(20)?,
                    rounded_font_size: row.get(21)?,
                    rounded_text_color: row.get(22)?,
                    rounded_background_color: row.get(23)?,
                    rounded_corner_radius: row.get(24)?,
                    rounded_padding_x: row.get(25)?,
                    rounded_padding_y: row.get(26)?,
                    rounded_margin_bottom: row.get(27)?,
                    rounded_line_spacing: row.get(28)?,
                    rounded_letter_spacing: row.get(29)?,
                    created_at: row.get(30)?,
                    updated_at: row.get(31)?,
                })
            })
            .map_err(|error| format!("无法读取字幕样式: {error}"))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| format!("无法解析字幕样式: {error}"))?;

        Ok(styles)
    })
}

#[tauri::command]
pub fn get_subtitle_style(
    store: tauri::State<'_, SettingsStore>,
    id: String,
) -> Result<SubtitleStyle, String> {
    store.with_connection(|connection| {
        connection
            .query_row(
                "
                SELECT
                    id, name, is_default,
                    render_mode, subtitle_layout, preview_text_mode,
                    primary_font_name, primary_font_size, primary_color,
                    primary_outline_color, primary_outline_width, primary_spacing,
                    primary_margin_bottom,
                    secondary_font_name, secondary_font_size, secondary_color,
                    secondary_outline_color, secondary_outline_width, secondary_spacing,
                    vertical_spacing,
                    rounded_font_name, rounded_font_size, rounded_text_color,
                    rounded_background_color, rounded_corner_radius, rounded_padding_x,
                    rounded_padding_y, rounded_margin_bottom, rounded_line_spacing,
                    rounded_letter_spacing,
                    created_at, updated_at
                FROM subtitle_styles
                WHERE id = ?1
                ",
                params![id],
                |row| {
                    Ok(SubtitleStyle {
                        id: row.get(0)?,
                        name: row.get(1)?,
                        is_default: row.get::<_, i64>(2)? != 0,
                        render_mode: row.get(3)?,
                        subtitle_layout: row.get(4)?,
                        preview_text_mode: row.get(5)?,
                        primary_font_name: row.get(6)?,
                        primary_font_size: row.get(7)?,
                        primary_color: row.get(8)?,
                        primary_outline_color: row.get(9)?,
                        primary_outline_width: row.get(10)?,
                        primary_spacing: row.get(11)?,
                        primary_margin_bottom: row.get(12)?,
                        secondary_font_name: row.get(13)?,
                        secondary_font_size: row.get(14)?,
                        secondary_color: row.get(15)?,
                        secondary_outline_color: row.get(16)?,
                        secondary_outline_width: row.get(17)?,
                        secondary_spacing: row.get(18)?,
                        vertical_spacing: row.get(19)?,
                        rounded_font_name: row.get(20)?,
                        rounded_font_size: row.get(21)?,
                        rounded_text_color: row.get(22)?,
                        rounded_background_color: row.get(23)?,
                        rounded_corner_radius: row.get(24)?,
                        rounded_padding_x: row.get(25)?,
                        rounded_padding_y: row.get(26)?,
                        rounded_margin_bottom: row.get(27)?,
                        rounded_line_spacing: row.get(28)?,
                        rounded_letter_spacing: row.get(29)?,
                        created_at: row.get(30)?,
                        updated_at: row.get(31)?,
                    })
                },
            )
            .map_err(|error| format!("无法获取字幕样式: {error}"))
    })
}

#[tauri::command]
pub fn create_subtitle_style(
    store: tauri::State<'_, SettingsStore>,
    request: CreateSubtitleStyleRequest,
) -> Result<SubtitleStyle, String> {
    let id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();

    store.with_connection(|connection| {
        connection
            .execute(
                "
                INSERT INTO subtitle_styles (
                    id, name, is_default,
                    render_mode, subtitle_layout, preview_text_mode,
                    primary_font_name, primary_font_size, primary_color,
                    primary_outline_color, primary_outline_width, primary_spacing,
                    primary_margin_bottom,
                    secondary_font_name, secondary_font_size, secondary_color,
                    secondary_outline_color, secondary_outline_width, secondary_spacing,
                    vertical_spacing,
                    rounded_font_name, rounded_font_size, rounded_text_color,
                    rounded_background_color, rounded_corner_radius, rounded_padding_x,
                    rounded_padding_y, rounded_margin_bottom, rounded_line_spacing,
                    rounded_letter_spacing,
                    created_at, updated_at
                )
                VALUES (
                    ?1, ?2, 0, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12,
                    ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23,
                    ?24, ?25, ?26, ?27, ?28, ?29, ?30, ?31
                )
                ",
                params![
                    id,
                    request.name,
                    request.render_mode,
                    request.subtitle_layout,
                    request.preview_text_mode,
                    request.primary_font_name,
                    request.primary_font_size,
                    request.primary_color,
                    request.primary_outline_color,
                    request.primary_outline_width,
                    request.primary_spacing,
                    request.primary_margin_bottom,
                    request.secondary_font_name,
                    request.secondary_font_size,
                    request.secondary_color,
                    request.secondary_outline_color,
                    request.secondary_outline_width,
                    request.secondary_spacing,
                    request.vertical_spacing,
                    request.rounded_font_name,
                    request.rounded_font_size,
                    request.rounded_text_color,
                    request.rounded_background_color,
                    request.rounded_corner_radius,
                    request.rounded_padding_x,
                    request.rounded_padding_y,
                    request.rounded_margin_bottom,
                    request.rounded_line_spacing,
                    request.rounded_letter_spacing,
                    now,
                    now,
                ],
            )
            .map_err(|error| format!("无法创建字幕样式: {error}"))?;

        // 查询刚创建的样式
        connection
            .query_row(
                "
                SELECT
                    id, name, is_default,
                    render_mode, subtitle_layout, preview_text_mode,
                    primary_font_name, primary_font_size, primary_color,
                    primary_outline_color, primary_outline_width, primary_spacing,
                    primary_margin_bottom,
                    secondary_font_name, secondary_font_size, secondary_color,
                    secondary_outline_color, secondary_outline_width, secondary_spacing,
                    vertical_spacing,
                    rounded_font_name, rounded_font_size, rounded_text_color,
                    rounded_background_color, rounded_corner_radius, rounded_padding_x,
                    rounded_padding_y, rounded_margin_bottom, rounded_line_spacing,
                    rounded_letter_spacing,
                    created_at, updated_at
                FROM subtitle_styles
                WHERE id = ?1
                ",
                params![id],
                |row| {
                    Ok(SubtitleStyle {
                        id: row.get(0)?,
                        name: row.get(1)?,
                        is_default: row.get::<_, i64>(2)? != 0,
                        render_mode: row.get(3)?,
                        subtitle_layout: row.get(4)?,
                        preview_text_mode: row.get(5)?,
                        primary_font_name: row.get(6)?,
                        primary_font_size: row.get(7)?,
                        primary_color: row.get(8)?,
                        primary_outline_color: row.get(9)?,
                        primary_outline_width: row.get(10)?,
                        primary_spacing: row.get(11)?,
                        primary_margin_bottom: row.get(12)?,
                        secondary_font_name: row.get(13)?,
                        secondary_font_size: row.get(14)?,
                        secondary_color: row.get(15)?,
                        secondary_outline_color: row.get(16)?,
                        secondary_outline_width: row.get(17)?,
                        secondary_spacing: row.get(18)?,
                        vertical_spacing: row.get(19)?,
                        rounded_font_name: row.get(20)?,
                        rounded_font_size: row.get(21)?,
                        rounded_text_color: row.get(22)?,
                        rounded_background_color: row.get(23)?,
                        rounded_corner_radius: row.get(24)?,
                        rounded_padding_x: row.get(25)?,
                        rounded_padding_y: row.get(26)?,
                        rounded_margin_bottom: row.get(27)?,
                        rounded_line_spacing: row.get(28)?,
                        rounded_letter_spacing: row.get(29)?,
                        created_at: row.get(30)?,
                        updated_at: row.get(31)?,
                    })
                },
            )
            .map_err(|error| format!("无法获取创建的字幕样式: {error}"))
    })
}

#[tauri::command]
pub fn update_subtitle_style(
    store: tauri::State<'_, SettingsStore>,
    request: UpdateSubtitleStyleRequest,
) -> Result<SubtitleStyle, String> {
    let request_id = request.id.clone();

    store.with_connection(|connection| {
        let now = chrono::Utc::now().to_rfc3339();

        connection
            .execute(
                "
                UPDATE subtitle_styles
                SET
                    name = ?2,
                    render_mode = ?3,
                    subtitle_layout = ?4,
                    preview_text_mode = ?5,
                    primary_font_name = ?6,
                    primary_font_size = ?7,
                    primary_color = ?8,
                    primary_outline_color = ?9,
                    primary_outline_width = ?10,
                    primary_spacing = ?11,
                    primary_margin_bottom = ?12,
                    secondary_font_name = ?13,
                    secondary_font_size = ?14,
                    secondary_color = ?15,
                    secondary_outline_color = ?16,
                    secondary_outline_width = ?17,
                    secondary_spacing = ?18,
                    vertical_spacing = ?19,
                    rounded_font_name = ?20,
                    rounded_font_size = ?21,
                    rounded_text_color = ?22,
                    rounded_background_color = ?23,
                    rounded_corner_radius = ?24,
                    rounded_padding_x = ?25,
                    rounded_padding_y = ?26,
                    rounded_margin_bottom = ?27,
                    rounded_line_spacing = ?28,
                    rounded_letter_spacing = ?29,
                    updated_at = ?30
                WHERE id = ?1
                ",
                params![
                    request.id,
                    request.name,
                    request.render_mode,
                    request.subtitle_layout,
                    request.preview_text_mode,
                    request.primary_font_name,
                    request.primary_font_size,
                    request.primary_color,
                    request.primary_outline_color,
                    request.primary_outline_width,
                    request.primary_spacing,
                    request.primary_margin_bottom,
                    request.secondary_font_name,
                    request.secondary_font_size,
                    request.secondary_color,
                    request.secondary_outline_color,
                    request.secondary_outline_width,
                    request.secondary_spacing,
                    request.vertical_spacing,
                    request.rounded_font_name,
                    request.rounded_font_size,
                    request.rounded_text_color,
                    request.rounded_background_color,
                    request.rounded_corner_radius,
                    request.rounded_padding_x,
                    request.rounded_padding_y,
                    request.rounded_margin_bottom,
                    request.rounded_line_spacing,
                    request.rounded_letter_spacing,
                    now,
                ],
            )
            .map_err(|error| format!("无法更新字幕样式: {error}"))?;

        // 查询更新后的样式
        connection
            .query_row(
                "
                SELECT
                    id, name, is_default,
                    render_mode, subtitle_layout, preview_text_mode,
                    primary_font_name, primary_font_size, primary_color,
                    primary_outline_color, primary_outline_width, primary_spacing,
                    primary_margin_bottom,
                    secondary_font_name, secondary_font_size, secondary_color,
                    secondary_outline_color, secondary_outline_width, secondary_spacing,
                    vertical_spacing,
                    rounded_font_name, rounded_font_size, rounded_text_color,
                    rounded_background_color, rounded_corner_radius, rounded_padding_x,
                    rounded_padding_y, rounded_margin_bottom, rounded_line_spacing,
                    rounded_letter_spacing,
                    created_at, updated_at
                FROM subtitle_styles
                WHERE id = ?1
                ",
                params![request_id],
                |row| {
                    Ok(SubtitleStyle {
                        id: row.get(0)?,
                        name: row.get(1)?,
                        is_default: row.get::<_, i64>(2)? != 0,
                        render_mode: row.get(3)?,
                        subtitle_layout: row.get(4)?,
                        preview_text_mode: row.get(5)?,
                        primary_font_name: row.get(6)?,
                        primary_font_size: row.get(7)?,
                        primary_color: row.get(8)?,
                        primary_outline_color: row.get(9)?,
                        primary_outline_width: row.get(10)?,
                        primary_spacing: row.get(11)?,
                        primary_margin_bottom: row.get(12)?,
                        secondary_font_name: row.get(13)?,
                        secondary_font_size: row.get(14)?,
                        secondary_color: row.get(15)?,
                        secondary_outline_color: row.get(16)?,
                        secondary_outline_width: row.get(17)?,
                        secondary_spacing: row.get(18)?,
                        vertical_spacing: row.get(19)?,
                        rounded_font_name: row.get(20)?,
                        rounded_font_size: row.get(21)?,
                        rounded_text_color: row.get(22)?,
                        rounded_background_color: row.get(23)?,
                        rounded_corner_radius: row.get(24)?,
                        rounded_padding_x: row.get(25)?,
                        rounded_padding_y: row.get(26)?,
                        rounded_margin_bottom: row.get(27)?,
                        rounded_line_spacing: row.get(28)?,
                        rounded_letter_spacing: row.get(29)?,
                        created_at: row.get(30)?,
                        updated_at: row.get(31)?,
                    })
                },
            )
            .map_err(|error| format!("无法获取更新后的字幕样式: {error}"))
    })
}

#[tauri::command]
pub fn delete_subtitle_style(
    store: tauri::State<'_, SettingsStore>,
    id: String,
) -> Result<(), String> {
    store.with_connection(|connection| {
        // 检查是否为默认样式
        let is_default: i64 = connection
            .query_row(
                "SELECT is_default FROM subtitle_styles WHERE id = ?1",
                params![id],
                |row| row.get(0),
            )
            .map_err(|error| format!("无法查询字幕样式: {error}"))?;

        if is_default != 0 {
            return Err("不能删除默认样式".to_string());
        }

        connection
            .execute("DELETE FROM subtitle_styles WHERE id = ?1", params![id])
            .map_err(|error| format!("无法删除字幕样式: {error}"))?;

        Ok(())
    })
}
