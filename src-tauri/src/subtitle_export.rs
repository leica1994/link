use crate::subtitle_style::SubtitleStyle;
use crate::transcription::{escape_ass_text, ms_to_ass_time, TranscriptionSegment};

const ASS_STYLE_FORMAT: &str = "Format: Name,Fontname,Fontsize,PrimaryColour,SecondaryColour,OutlineColour,BackColour,Bold,Italic,Underline,StrikeOut,ScaleX,ScaleY,Spacing,Angle,BorderStyle,Outline,Shadow,Alignment,MarginL,MarginR,MarginV,Encoding";

pub(crate) fn serialize_styled_ass(
    segments: &[TranscriptionSegment],
    style: &SubtitleStyle,
) -> String {
    let mut content = ass_header(style, false);
    for segment in segments {
        push_dialogue(&mut content, segment, "Primary");
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
    let mut content = ass_header(style, is_bilingual);

    for (source, translated) in source_segments.iter().zip(translated_segments.iter()) {
        match layout {
            "source-only" => push_dialogue(&mut content, source, "Primary"),
            "target-only" => push_dialogue(&mut content, translated, "Primary"),
            "source-above" => {
                push_dialogue(&mut content, source, "Primary");
                push_dialogue(&mut content, translated, "Secondary");
            }
            _ => {
                push_dialogue(&mut content, translated, "Primary");
                push_dialogue(&mut content, source, "Secondary");
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
    let bottom_margin = if style.render_mode == "rounded" {
        style.rounded_margin_bottom
    } else {
        style.primary_margin_bottom
    };
    let line_spacing = if style.render_mode == "rounded" {
        style.rounded_line_spacing
    } else {
        style.vertical_spacing
    };
    let secondary_size = if style.render_mode == "rounded" {
        style.rounded_font_size
    } else {
        style.secondary_font_size
    };
    let primary_margin = if is_bilingual {
        bottom_margin
            .saturating_add(secondary_size)
            .saturating_add(line_spacing)
    } else {
        bottom_margin
    };

    if style.render_mode == "rounded" {
        let font_name = sanitize_ass_field(&style.rounded_font_name, "Microsoft YaHei");
        let text_color = ass_color(&style.rounded_text_color, "#FFFFFF");
        let background_color = ass_color(&style.rounded_background_color, "#191919CC");
        let box_padding = style.rounded_padding_x.max(style.rounded_padding_y).max(1) as f64;
        let primary = ass_style_line(
            "Default",
            &font_name,
            style.rounded_font_size,
            &text_color,
            &background_color,
            &background_color,
            style.rounded_letter_spacing as f64,
            3,
            box_padding,
            primary_margin,
        );
        let secondary = ass_style_line(
            "Secondary",
            &font_name,
            style.rounded_font_size,
            &text_color,
            &background_color,
            &background_color,
            style.rounded_letter_spacing as f64,
            3,
            box_padding,
            bottom_margin,
        );
        return (primary, secondary);
    }

    let primary = ass_style_line(
        "Default",
        &sanitize_ass_field(&style.primary_font_name, "Microsoft YaHei"),
        style.primary_font_size,
        &ass_color(&style.primary_color, "#FFFFFF"),
        &ass_color(&style.primary_outline_color, "#000000"),
        "&H00000000",
        style.primary_spacing,
        1,
        style.primary_outline_width.max(0.0),
        primary_margin,
    );
    let secondary = ass_style_line(
        "Secondary",
        &sanitize_ass_field(&style.secondary_font_name, "Microsoft YaHei"),
        style.secondary_font_size,
        &ass_color(&style.secondary_color, "#FFFFFF"),
        &ass_color(&style.secondary_outline_color, "#000000"),
        "&H00000000",
        style.secondary_spacing,
        1,
        style.secondary_outline_width.max(0.0),
        bottom_margin,
    );
    (primary, secondary)
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

fn push_dialogue(content: &mut String, segment: &TranscriptionSegment, style_name: &str) {
    let text = segment.text.trim();
    if text.is_empty() {
        return;
    }

    let ass_style_name = if style_name == "Primary" {
        "Default"
    } else {
        style_name
    };
    content.push_str(&format!(
        "Dialogue: 0,{},{},{ass_style_name},,0,0,0,,{}\n",
        ms_to_ass_time(segment.start_time),
        ms_to_ass_time(segment.end_time),
        escape_ass_text(text),
    ));
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
}
