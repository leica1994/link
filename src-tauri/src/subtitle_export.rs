use crate::subtitle_style::SubtitleStyle;
use crate::transcription::{escape_ass_text, ms_to_ass_time, TranscriptionSegment};

const ASS_STYLE_FORMAT: &str = "Format: Name,Fontname,Fontsize,PrimaryColour,SecondaryColour,OutlineColour,BackColour,Bold,Italic,Underline,StrikeOut,ScaleX,ScaleY,Spacing,Angle,BorderStyle,Outline,Shadow,Alignment,MarginL,MarginR,MarginV,Encoding";
const ASS_PLAY_RES_X: f64 = 1280.0;
const ASS_STYLE_MARGIN_X: f64 = 10.0;
const ASS_WRAP_SAFE_PADDING: f64 = 48.0;
const ASS_LINE_HEIGHT_RATIO: f64 = 1.15;
const ASS_EXTRA_LINE_GAP: f64 = 4.0;

#[derive(Debug, Clone, Copy)]
struct AssDialogueStyle {
    font_size: u32,
    spacing: f64,
    outline: f64,
}

#[derive(Debug, Clone, Copy)]
struct AssLayoutMetrics {
    primary: AssDialogueStyle,
    secondary: AssDialogueStyle,
    primary_margin: u32,
}

#[derive(Debug)]
struct RenderedDialogue {
    text: String,
    line_count: usize,
}

pub(crate) fn serialize_styled_ass(
    segments: &[TranscriptionSegment],
    style: &SubtitleStyle,
) -> String {
    let metrics = build_ass_layout_metrics(style, false);
    let mut content = ass_header(style, false);
    for segment in segments {
        push_dialogue(&mut content, segment, "Primary", 0, 0, &metrics.primary);
    }
    content
}

pub(crate) fn serialize_styled_bilingual_ass(
    source_segments: &[TranscriptionSegment],
    translated_segments: &[TranscriptionSegment],
    style: &SubtitleStyle,
) -> String {
    let layout = style.subtitle_layout.as_str();
    let is_bilingual = matches!(layout, "target-above" | "source-above");
    let metrics = build_ass_layout_metrics(style, is_bilingual);
    let mut content = ass_header(style, is_bilingual);

    for (source, translated) in source_segments.iter().zip(translated_segments.iter()) {
        match layout {
            "source-only" => push_dialogue(&mut content, source, "Primary", 0, 0, &metrics.primary),
            "target-only" => {
                push_dialogue(&mut content, translated, "Primary", 0, 0, &metrics.primary)
            }
            "source-above" => {
                push_bilingual_dialogues(&mut content, source, translated, &metrics);
            }
            _ => {
                push_bilingual_dialogues(&mut content, translated, source, &metrics);
            }
        }
    }

    content
}

fn ass_header(style: &SubtitleStyle, is_bilingual: bool) -> String {
    let (primary, secondary) = build_ass_styles(style, is_bilingual);
    format!(
        "[Script Info]\n\
         Author: Link\n\
         ScriptType: v4.00+\n\
         Collisions: Normal\n\
         PlayResX: 1280\n\
         PlayResY: 720\n\
         WrapStyle: 0\n\
         ScaledBorderAndShadow: yes\n\n\
         [V4+ Styles]\n\
         {ASS_STYLE_FORMAT}\n\
         {primary}\n\
         {secondary}\n\n\
         [Events]\n\
         Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text\n"
    )
}

fn build_ass_styles(style: &SubtitleStyle, is_bilingual: bool) -> (String, String) {
    let metrics = build_ass_layout_metrics(style, is_bilingual);

    if style.render_mode == "rounded" {
        let font_name = sanitize_ass_field(&style.rounded_font_name, "Microsoft YaHei");
        let text_color = ass_color(&style.rounded_text_color, "#FFFFFF");
        let background_color = ass_color(&style.rounded_background_color, "#191919CC");
        let primary = ass_style_line(
            "Default",
            &font_name,
            metrics.primary.font_size,
            &text_color,
            &background_color,
            &background_color,
            metrics.primary.spacing,
            3,
            metrics.primary.outline,
            metrics.primary_margin,
        );
        let secondary = ass_style_line(
            "Secondary",
            &font_name,
            metrics.secondary.font_size,
            &text_color,
            &background_color,
            &background_color,
            metrics.secondary.spacing,
            3,
            metrics.secondary.outline,
            secondary_margin(style),
        );
        return (primary, secondary);
    }

    let primary = ass_style_line(
        "Default",
        &sanitize_ass_field(&style.primary_font_name, "Microsoft YaHei"),
        metrics.primary.font_size,
        &ass_color(&style.primary_color, "#FFFFFF"),
        &ass_color(&style.primary_outline_color, "#000000"),
        "&H00000000",
        metrics.primary.spacing,
        1,
        metrics.primary.outline,
        metrics.primary_margin,
    );
    let secondary = ass_style_line(
        "Secondary",
        &sanitize_ass_field(&style.secondary_font_name, "Microsoft YaHei"),
        metrics.secondary.font_size,
        &ass_color(&style.secondary_color, "#FFFFFF"),
        &ass_color(&style.secondary_outline_color, "#000000"),
        "&H00000000",
        metrics.secondary.spacing,
        1,
        metrics.secondary.outline,
        secondary_margin(style),
    );
    (primary, secondary)
}

fn build_ass_layout_metrics(style: &SubtitleStyle, is_bilingual: bool) -> AssLayoutMetrics {
    let bottom_margin = secondary_margin(style);
    let line_spacing = if style.render_mode == "rounded" {
        style.rounded_line_spacing
    } else {
        style.vertical_spacing
    };

    let (primary, secondary) = if style.render_mode == "rounded" {
        let box_padding = style.rounded_padding_x.max(style.rounded_padding_y).max(1) as f64;
        let rounded_style = AssDialogueStyle {
            font_size: style.rounded_font_size,
            spacing: style.rounded_letter_spacing as f64,
            outline: box_padding,
        };
        (rounded_style, rounded_style)
    } else {
        (
            AssDialogueStyle {
                font_size: style.primary_font_size,
                spacing: style.primary_spacing,
                outline: style.primary_outline_width.max(0.0),
            },
            AssDialogueStyle {
                font_size: style.secondary_font_size,
                spacing: style.secondary_spacing,
                outline: style.secondary_outline_width.max(0.0),
            },
        )
    };

    let primary_margin = if is_bilingual {
        bottom_margin
            .saturating_add(secondary.font_size)
            .saturating_add(line_spacing)
    } else {
        bottom_margin
    };

    AssLayoutMetrics {
        primary,
        secondary,
        primary_margin,
    }
}

fn secondary_margin(style: &SubtitleStyle) -> u32 {
    if style.render_mode == "rounded" {
        style.rounded_margin_bottom
    } else {
        style.primary_margin_bottom
    }
}

#[allow(clippy::too_many_arguments)]
fn ass_style_line(
    name: &str,
    font_name: &str,
    font_size: u32,
    primary_color: &str,
    outline_color: &str,
    back_color: &str,
    spacing: f64,
    border_style: u8,
    outline: f64,
    margin_v: u32,
) -> String {
    format!(
        "Style: {name},{font_name},{},{primary_color},&H000000FF,{outline_color},{back_color},-1,0,0,0,100,100,{:.2},0,{border_style},{:.2},0,2,10,10,{margin_v},1",
        font_size.max(1),
        spacing.max(0.0),
        outline.max(0.0),
    )
}

fn push_bilingual_dialogues(
    content: &mut String,
    primary_segment: &TranscriptionSegment,
    secondary_segment: &TranscriptionSegment,
    metrics: &AssLayoutMetrics,
) {
    let primary = render_dialogue(primary_segment, &metrics.primary);
    let secondary = render_dialogue(secondary_segment, &metrics.secondary);
    let primary_margin_v = secondary
        .as_ref()
        .and_then(|dialogue| primary_margin_override(metrics, dialogue.line_count))
        .unwrap_or(0);

    if let Some(dialogue) = primary {
        push_rendered_dialogue(
            content,
            primary_segment,
            "Primary",
            1,
            primary_margin_v,
            &dialogue,
        );
    }
    if let Some(dialogue) = secondary {
        push_rendered_dialogue(content, secondary_segment, "Secondary", 0, 0, &dialogue);
    }
}

fn push_dialogue(
    content: &mut String,
    segment: &TranscriptionSegment,
    style_name: &str,
    layer: u8,
    margin_v: u32,
    style: &AssDialogueStyle,
) {
    if let Some(dialogue) = render_dialogue(segment, style) {
        push_rendered_dialogue(content, segment, style_name, layer, margin_v, &dialogue);
    }
}

fn render_dialogue(
    segment: &TranscriptionSegment,
    style: &AssDialogueStyle,
) -> Option<RenderedDialogue> {
    let text = segment.text.trim();
    if text.is_empty() {
        return None;
    }

    let wrapped_text = wrap_text_for_ass(text, style);
    let line_count = wrapped_text.lines().count().max(1);
    Some(RenderedDialogue {
        text: wrapped_text,
        line_count,
    })
}

fn push_rendered_dialogue(
    content: &mut String,
    segment: &TranscriptionSegment,
    style_name: &str,
    layer: u8,
    margin_v: u32,
    dialogue: &RenderedDialogue,
) {
    let ass_style_name = if style_name == "Primary" {
        "Default"
    } else {
        style_name
    };
    content.push_str(&format!(
        "Dialogue: {layer},{},{},{ass_style_name},,0,0,{margin_v},,{}\n",
        ms_to_ass_time(segment.start_time),
        ms_to_ass_time(segment.end_time),
        escape_ass_text(&dialogue.text),
    ));
}

fn primary_margin_override(metrics: &AssLayoutMetrics, secondary_line_count: usize) -> Option<u32> {
    let extra_lines = secondary_line_count.checked_sub(1)?;
    if extra_lines == 0 {
        return None;
    }

    let extra_line_height = metrics.secondary.font_size as f64 * ASS_LINE_HEIGHT_RATIO
        + metrics.secondary.outline * 2.0
        + ASS_EXTRA_LINE_GAP;
    Some(
        metrics
            .primary_margin
            .saturating_add((extra_lines as f64 * extra_line_height).ceil() as u32),
    )
}

fn wrap_text_for_ass(text: &str, style: &AssDialogueStyle) -> String {
    let normalized = text.replace("\r\n", "\n").replace('\r', "\n");
    normalized
        .split('\n')
        .flat_map(|line| wrap_plain_line(line, style))
        .collect::<Vec<_>>()
        .join("\n")
}

fn wrap_plain_line(line: &str, style: &AssDialogueStyle) -> Vec<String> {
    let line = line.trim();
    if line.is_empty() {
        return vec![String::new()];
    }

    let max_width = max_ass_line_width(style);
    if estimate_ass_text_width(line, style) <= max_width {
        return vec![line.to_string()];
    }

    if estimate_ass_text_width(line, style) <= max_width * 2.0 {
        if let Some((left, right)) = split_balanced_line(line, style, max_width) {
            return vec![left, right];
        }
    }

    wrap_line_greedily(line, style, max_width)
}

fn wrap_line_greedily(line: &str, style: &AssDialogueStyle, max_width: f64) -> Vec<String> {
    let mut remaining = line.trim().to_string();
    let mut lines = Vec::new();

    while estimate_ass_text_width(&remaining, style) > max_width {
        let Some(split_index) = find_greedy_split_index(&remaining, style, max_width) else {
            break;
        };
        let left = remaining[..split_index].trim_end().to_string();
        let right = remaining[split_index..].trim_start().to_string();
        if left.is_empty() || right.is_empty() {
            break;
        }
        lines.push(left);
        remaining = right;
    }

    if !remaining.is_empty() {
        lines.push(remaining);
    }
    lines
}

fn split_balanced_line(
    line: &str,
    style: &AssDialogueStyle,
    max_width: f64,
) -> Option<(String, String)> {
    let mut best: Option<(usize, f64)> = None;

    for index in split_boundaries(line) {
        let left = line[..index].trim_end();
        let right = line[index..].trim_start();
        if left.is_empty() || right.is_empty() {
            continue;
        }

        let left_width = estimate_ass_text_width(left, style);
        let right_width = estimate_ass_text_width(right, style);
        if left_width > max_width || right_width > max_width {
            continue;
        }

        let score = (left_width - right_width).abs() + break_penalty(line, index, style);
        if best.map_or(true, |(_, best_score)| score < best_score) {
            best = Some((index, score));
        }
    }

    best.map(|(index, _)| {
        (
            line[..index].trim_end().to_string(),
            line[index..].trim_start().to_string(),
        )
    })
}

fn find_greedy_split_index(line: &str, style: &AssDialogueStyle, max_width: f64) -> Option<usize> {
    let min_width = max_width * 0.45;
    let mut fallback = None;
    let mut best_break: Option<(usize, f64)> = None;

    for index in split_boundaries(line) {
        let left = line[..index].trim_end();
        if left.is_empty() {
            continue;
        }

        let width = estimate_ass_text_width(left, style);
        if width > max_width {
            break;
        }

        fallback = Some(index);
        if width >= min_width {
            let score = max_width - width + break_penalty(line, index, style);
            if best_break.map_or(true, |(_, best_score)| score < best_score) {
                best_break = Some((index, score));
            }
        }
    }

    best_break.map(|(index, _)| index).or(fallback)
}

fn split_boundaries(line: &str) -> Vec<usize> {
    line.char_indices()
        .skip(1)
        .map(|(index, _)| index)
        .chain(std::iter::once(line.len()))
        .collect()
}

fn break_penalty(line: &str, index: usize, style: &AssDialogueStyle) -> f64 {
    let previous = line[..index].chars().next_back();
    let next = line[index..].chars().next();
    let font_size = style.font_size as f64;

    let mut penalty = font_size * 0.2;
    if previous.is_some_and(is_strong_break_char) {
        penalty -= font_size * 0.9;
    } else if previous.is_some_and(|character| character.is_whitespace())
        || next.is_some_and(|character| character.is_whitespace())
    {
        penalty -= font_size * 0.6;
    }
    if next.is_some_and(is_bad_line_start_char) {
        penalty += font_size * 2.0;
    }
    penalty
}

fn max_ass_line_width(style: &AssDialogueStyle) -> f64 {
    let reserved = (ASS_STYLE_MARGIN_X + style.outline.max(0.0)) * 2.0 + ASS_WRAP_SAFE_PADDING;
    (ASS_PLAY_RES_X - reserved).max(style.font_size.max(1) as f64 * 4.0)
}

fn estimate_ass_text_width(text: &str, style: &AssDialogueStyle) -> f64 {
    let spacing = style.spacing.max(0.0);
    let font_size = style.font_size.max(1) as f64;
    let mut width = 0.0;
    let mut visible_chars = 0usize;

    for character in text.chars() {
        width += ass_character_width_factor(character) * font_size;
        visible_chars += 1;
    }

    if visible_chars > 1 {
        width += spacing * (visible_chars - 1) as f64;
    }
    width
}

fn ass_character_width_factor(character: char) -> f64 {
    if character.is_ascii_whitespace() {
        0.35
    } else if character.is_ascii_punctuation() {
        0.36
    } else if character.is_ascii() {
        0.56
    } else if is_full_width_character(character) {
        1.0
    } else {
        0.8
    }
}

fn is_full_width_character(character: char) -> bool {
    matches!(
        character as u32,
        0x1100..=0x11FF
            | 0x2E80..=0xA4CF
            | 0xAC00..=0xD7A3
            | 0xF900..=0xFAFF
            | 0xFE10..=0xFE6F
            | 0xFF00..=0xFF60
            | 0xFFE0..=0xFFE6
    )
}

fn is_strong_break_char(character: char) -> bool {
    matches!(
        character,
        ' ' | ','
            | '.'
            | '?'
            | '!'
            | ';'
            | ':'
            | '，'
            | '。'
            | '？'
            | '！'
            | '；'
            | '：'
            | '、'
            | '…'
    )
}

fn is_bad_line_start_char(character: char) -> bool {
    matches!(
        character,
        ',' | '.'
            | '?'
            | '!'
            | ';'
            | ':'
            | ')'
            | ']'
            | '}'
            | '，'
            | '。'
            | '？'
            | '！'
            | '；'
            | '：'
            | '、'
            | '）'
            | '】'
            | '》'
            | '」'
            | '』'
    )
}

fn sanitize_ass_field(value: &str, fallback: &str) -> String {
    let value = value.trim();
    if value.is_empty() {
        fallback.to_string()
    } else {
        value.replace([',', '\n', '\r'], " ")
    }
}

fn ass_color(value: &str, fallback: &str) -> String {
    let normalized = normalize_hex_color(value).or_else(|| normalize_hex_color(fallback));
    let Some((red, green, blue, alpha)) = normalized else {
        return "&H00FFFFFF".to_string();
    };
    let ass_alpha = 255u8.saturating_sub(alpha);
    format!("&H{ass_alpha:02X}{blue:02X}{green:02X}{red:02X}")
}

fn normalize_hex_color(value: &str) -> Option<(u8, u8, u8, u8)> {
    let value = value.trim().strip_prefix('#')?;
    if value.len() != 6 && value.len() != 8 {
        return None;
    }

    let red = u8::from_str_radix(&value[0..2], 16).ok()?;
    let green = u8::from_str_radix(&value[2..4], 16).ok()?;
    let blue = u8::from_str_radix(&value[4..6], 16).ok()?;
    let alpha = if value.len() == 8 {
        u8::from_str_radix(&value[6..8], 16).ok()?
    } else {
        255
    };
    Some((red, green, blue, alpha))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_rgba_to_ass_alpha_and_bgr() {
        assert_eq!(ass_color("#112233CC", "#FFFFFF"), "&H33332211");
        assert_eq!(ass_color("#FFFFFF", "#000000"), "&H00FFFFFF");
    }

    #[test]
    fn bilingual_ass_uses_separate_layers_for_primary_and_secondary() {
        let style = test_style(48, 30);
        let output = serialize_styled_bilingual_ass(
            &[segment("short source")],
            &[segment("短译文")],
            &style,
        );

        assert!(output.contains("Dialogue: 1,0:00:00.00,0:00:01.00,Default"));
        assert!(output.contains("Dialogue: 0,0:00:00.00,0:00:01.00,Secondary"));
    }

    #[test]
    fn wraps_long_primary_text_when_style_is_large() {
        let style = test_style(50, 30);
        let output = serialize_styled_bilingual_ass(
            &[segment("short source")],
            &[segment(
                "在中国中部的深山里，藏着一个有着数百年历史的洞穴村落，如今早已人去楼空。",
            )],
            &style,
        );

        assert!(default_dialogue(&output).contains("\\N"));
    }

    #[test]
    fn keeps_long_primary_text_on_one_line_when_style_is_small_enough() {
        let style = test_style(24, 30);
        let output = serialize_styled_bilingual_ass(
            &[segment("short source")],
            &[segment(
                "在中国中部的深山里，藏着一个有着数百年历史的洞穴村落，如今早已人去楼空。",
            )],
            &style,
        );

        assert!(!default_dialogue(&output).contains("\\N"));
    }

    #[test]
    fn raises_primary_dialogue_when_secondary_wraps() {
        let style = test_style(48, 30);
        let output = serialize_styled_bilingual_ass(
            &[segment(
                "Hidden in the mountains of central China lies a centuries-old cave village that today is totally abandoned.",
            )],
            &[segment("短译文")],
            &style,
        );
        let default_line = default_dialogue(&output);
        let margin_v = dialogue_margin_v(default_line);

        assert!(secondary_dialogue(&output).contains("\\N"));
        assert!(margin_v > 0);
        assert!(margin_v > 48 + 30 + 15);
    }

    fn segment(text: &str) -> TranscriptionSegment {
        TranscriptionSegment {
            text: text.to_string(),
            start_time: 0,
            end_time: 1000,
            uid: String::new(),
            status: String::new(),
            words: Vec::new(),
        }
    }

    fn test_style(primary_font_size: u32, secondary_font_size: u32) -> SubtitleStyle {
        SubtitleStyle {
            id: "test".to_string(),
            name: "Test".to_string(),
            is_default: false,
            render_mode: "ass".to_string(),
            subtitle_layout: "target-above".to_string(),
            preview_text_mode: "medium".to_string(),
            primary_font_name: "Microsoft YaHei".to_string(),
            primary_font_size,
            primary_color: "#FFFFFF".to_string(),
            primary_outline_color: "#000000".to_string(),
            primary_outline_width: 2.0,
            primary_spacing: 0.0,
            primary_margin_bottom: 48,
            secondary_font_name: "Microsoft YaHei".to_string(),
            secondary_font_size,
            secondary_color: "#FFFFFF".to_string(),
            secondary_outline_color: "#000000".to_string(),
            secondary_outline_width: 2.0,
            secondary_spacing: 0.0,
            vertical_spacing: 15,
            rounded_font_name: "Microsoft YaHei".to_string(),
            rounded_font_size: 34,
            rounded_text_color: "#FFFFFF".to_string(),
            rounded_background_color: "#191919CC".to_string(),
            rounded_corner_radius: 14,
            rounded_padding_x: 24,
            rounded_padding_y: 14,
            rounded_margin_bottom: 60,
            rounded_line_spacing: 10,
            rounded_letter_spacing: 0,
            created_at: String::new(),
            updated_at: String::new(),
        }
    }

    fn default_dialogue(output: &str) -> &str {
        dialogue_line(output, ",Default,")
    }

    fn secondary_dialogue(output: &str) -> &str {
        dialogue_line(output, ",Secondary,")
    }

    fn dialogue_line<'a>(output: &'a str, style_marker: &str) -> &'a str {
        output
            .lines()
            .find(|line| line.starts_with("Dialogue:") && line.contains(style_marker))
            .expect("dialogue line should exist")
    }

    fn dialogue_margin_v(line: &str) -> u32 {
        line.split(',')
            .nth(7)
            .expect("dialogue margin should exist")
            .parse()
            .expect("dialogue margin should be numeric")
    }
}
