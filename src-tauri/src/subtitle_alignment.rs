use crate::transcription::{TranscriptionSegment, TranscriptionWord};
use serde::Serialize;

const ALIGNMENT_VERSION: &str = "downloaded-subtitle-align-v2";
const MIN_TOKEN_COVERAGE: f64 = 0.55;
const MIN_SEGMENT_COVERAGE: f64 = 0.60;
const MIN_SEGMENT_DURATION_MS: u64 = 300;
const DEFAULT_GAP_MS: u64 = 120;
const MAX_ALIGNMENT_MATRIX_CELLS: usize = 25_000_000;
const CHUNK_SUBTITLE_TOKENS: usize = 1_600;
const CHUNK_ASR_PADDING_TOKENS: usize = 800;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SubtitleAlignmentReport {
    pub alignment_version: String,
    pub token_coverage: f64,
    pub segment_coverage: f64,
    pub matched_token_count: usize,
    pub subtitle_token_count: usize,
    pub aligned_segment_count: usize,
    pub subtitle_segment_count: usize,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct SubtitleAlignmentResult {
    pub segments: Vec<TranscriptionSegment>,
    pub report: SubtitleAlignmentReport,
}

#[derive(Debug, Clone)]
struct TimedToken {
    normalized: String,
    start_time: u64,
    end_time: u64,
}

#[derive(Debug, Clone)]
struct SubtitleToken {
    normalized: String,
    segment_index: usize,
}

pub fn alignment_version() -> &'static str {
    ALIGNMENT_VERSION
}

pub fn align_downloaded_subtitles(
    downloaded_segments: &[TranscriptionSegment],
    asr_segments: &[TranscriptionSegment],
) -> Result<SubtitleAlignmentResult, String> {
    if downloaded_segments.is_empty() {
        return Err("下载字幕没有可用文本".to_string());
    }

    let subtitle_tokens = subtitle_tokens(downloaded_segments);
    let asr_tokens = asr_tokens(asr_segments);
    if subtitle_tokens.is_empty() {
        return Err("下载字幕没有可对齐文本".to_string());
    }
    if asr_tokens.is_empty() {
        return Err("ASR 没有返回词级时间戳，无法对齐下载字幕".to_string());
    }

    let matches = monotonic_match(&subtitle_tokens, &asr_tokens);
    let matched_count = matches
        .iter()
        .filter(|match_index| match_index.is_some())
        .count();
    let token_coverage = ratio(matched_count, subtitle_tokens.len());

    let mut segment_matched_tokens: Vec<Vec<usize>> = vec![Vec::new(); downloaded_segments.len()];
    for (token_index, token) in subtitle_tokens.iter().enumerate() {
        if let Some(asr_index) = matches[token_index] {
            segment_matched_tokens[token.segment_index].push(asr_index);
        }
    }

    let mut aligned_segments = Vec::with_capacity(downloaded_segments.len());
    let mut aligned_count = 0usize;
    for (index, segment) in downloaded_segments.iter().enumerate() {
        let matched = &segment_matched_tokens[index];
        if matched.is_empty() {
            aligned_segments.push(segment.clone());
            continue;
        }

        let start_asr_index = *matched.first().unwrap_or(&0);
        let end_asr_index = *matched.last().unwrap_or(&start_asr_index);
        let mut aligned = segment.clone();
        aligned.start_time = asr_tokens
            .get(start_asr_index)
            .map(|token| token.start_time)
            .unwrap_or(segment.start_time);
        aligned.end_time = asr_tokens
            .get(end_asr_index)
            .map(|token| token.end_time)
            .unwrap_or(segment.end_time);
        aligned.words =
            aligned_words_for_segment(&aligned.text, aligned.start_time, aligned.end_time);
        aligned.uid.clear();
        aligned.status.clear();
        aligned_segments.push(aligned);
        aligned_count += 1;
    }

    interpolate_unmatched_segments(&mut aligned_segments, &segment_matched_tokens);
    normalize_segment_timeline(&mut aligned_segments);

    let segment_coverage = ratio(aligned_count, downloaded_segments.len());
    let mut warnings = Vec::new();
    if token_coverage < MIN_TOKEN_COVERAGE {
        warnings.push(format!(
            "下载字幕 token 对齐覆盖率过低（{:.0}%）",
            token_coverage * 100.0
        ));
    }
    if segment_coverage < MIN_SEGMENT_COVERAGE {
        warnings.push(format!(
            "下载字幕行对齐覆盖率过低（{:.0}%）",
            segment_coverage * 100.0
        ));
    }

    let report = SubtitleAlignmentReport {
        alignment_version: ALIGNMENT_VERSION.to_string(),
        token_coverage,
        segment_coverage,
        matched_token_count: matched_count,
        subtitle_token_count: subtitle_tokens.len(),
        aligned_segment_count: aligned_count,
        subtitle_segment_count: downloaded_segments.len(),
        warnings,
    };

    if report.token_coverage < MIN_TOKEN_COVERAGE || report.segment_coverage < MIN_SEGMENT_COVERAGE
    {
        return Err(format!(
            "下载字幕对齐置信度不足，token 覆盖率 {:.0}%，行覆盖率 {:.0}%",
            report.token_coverage * 100.0,
            report.segment_coverage * 100.0
        ));
    }

    Ok(SubtitleAlignmentResult {
        segments: aligned_segments,
        report,
    })
}

fn subtitle_tokens(segments: &[TranscriptionSegment]) -> Vec<SubtitleToken> {
    let mut tokens = Vec::new();
    for (segment_index, segment) in segments.iter().enumerate() {
        for normalized in normalized_tokens(&segment.text) {
            tokens.push(SubtitleToken {
                normalized,
                segment_index,
            });
        }
    }
    tokens
}

fn asr_tokens(segments: &[TranscriptionSegment]) -> Vec<TimedToken> {
    let mut tokens = Vec::new();
    for segment in segments {
        let words = if segment.words.is_empty() {
            aligned_words_for_segment(&segment.text, segment.start_time, segment.end_time)
        } else {
            segment.words.clone()
        };
        for word in words {
            for normalized in normalized_tokens(&word.text) {
                tokens.push(TimedToken {
                    normalized,
                    start_time: word.start_time,
                    end_time: word.end_time.max(word.start_time.saturating_add(1)),
                });
            }
        }
    }
    tokens
}

fn monotonic_match(
    subtitle_tokens: &[SubtitleToken],
    asr_tokens: &[TimedToken],
) -> Vec<Option<usize>> {
    if subtitle_tokens.is_empty() || asr_tokens.is_empty() {
        return vec![None; subtitle_tokens.len()];
    }

    let matrix_cells = subtitle_tokens
        .len()
        .saturating_add(1)
        .saturating_mul(asr_tokens.len().saturating_add(1));
    if matrix_cells <= MAX_ALIGNMENT_MATRIX_CELLS {
        return lcs_match_range(subtitle_tokens, asr_tokens, 0);
    }

    chunked_lcs_match(subtitle_tokens, asr_tokens)
}

fn chunked_lcs_match(
    subtitle_tokens: &[SubtitleToken],
    asr_tokens: &[TimedToken],
) -> Vec<Option<usize>> {
    let mut matches = vec![None; subtitle_tokens.len()];
    let ratio = asr_tokens.len() as f64 / subtitle_tokens.len().max(1) as f64;
    let mut asr_cursor = 0usize;
    let mut subtitle_start = 0usize;

    while subtitle_start < subtitle_tokens.len() {
        let subtitle_end = (subtitle_start + CHUNK_SUBTITLE_TOKENS).min(subtitle_tokens.len());
        let estimated_asr_end = ((subtitle_end as f64) * ratio).ceil() as usize;
        let asr_start = asr_cursor.saturating_sub(CHUNK_ASR_PADDING_TOKENS / 4);
        let asr_end = estimated_asr_end
            .max(asr_cursor + CHUNK_SUBTITLE_TOKENS)
            .saturating_add(CHUNK_ASR_PADDING_TOKENS)
            .min(asr_tokens.len());

        if asr_start >= asr_end {
            break;
        }

        let chunk_matches = lcs_match_range(
            &subtitle_tokens[subtitle_start..subtitle_end],
            &asr_tokens[asr_start..asr_end],
            asr_start,
        );
        let mut last_match = None;
        for (offset, matched_asr_index) in chunk_matches.into_iter().enumerate() {
            if let Some(asr_index) = matched_asr_index {
                matches[subtitle_start + offset] = Some(asr_index);
                last_match = Some(asr_index);
            }
        }

        asr_cursor = last_match
            .map(|index| index.saturating_add(1))
            .unwrap_or_else(|| {
                asr_cursor
                    .saturating_add(((subtitle_end - subtitle_start) as f64 * ratio) as usize)
                    .max(asr_start.saturating_add(1))
            })
            .min(asr_tokens.len());
        subtitle_start = subtitle_end;
    }

    matches
}

fn lcs_match_range(
    subtitle_tokens: &[SubtitleToken],
    asr_tokens: &[TimedToken],
    asr_offset: usize,
) -> Vec<Option<usize>> {
    let asr_len = asr_tokens.len();
    let stride = asr_len + 1;
    let mut previous_row = vec![0u32; stride];
    let mut current_row = vec![0u32; stride];
    let mut directions = vec![0u8; (subtitle_tokens.len() + 1) * stride];

    for subtitle_index in 1..=subtitle_tokens.len() {
        current_row[0] = 0;
        let row_offset = subtitle_index * stride;
        directions[row_offset] = 2;
        let subtitle_token = &subtitle_tokens[subtitle_index - 1].normalized;

        for asr_index in 1..=asr_len {
            let diag = if subtitle_token == &asr_tokens[asr_index - 1].normalized {
                previous_row[asr_index - 1] + 1
            } else {
                0
            };
            let up = previous_row[asr_index];
            let left = current_row[asr_index - 1];

            let (score, direction) = if diag > up && diag > left {
                (diag, 1)
            } else if up >= left {
                (up, 2)
            } else {
                (left, 3)
            };

            current_row[asr_index] = score;
            directions[row_offset + asr_index] = direction;
        }

        std::mem::swap(&mut previous_row, &mut current_row);
    }

    let mut matches = vec![None; subtitle_tokens.len()];
    let mut subtitle_index = subtitle_tokens.len();
    let mut asr_index = asr_len;

    while subtitle_index > 0 && asr_index > 0 {
        match directions[subtitle_index * stride + asr_index] {
            1 => {
                matches[subtitle_index - 1] = Some(asr_offset + asr_index - 1);
                subtitle_index -= 1;
                asr_index -= 1;
            }
            2 => {
                subtitle_index -= 1;
            }
            3 => {
                asr_index -= 1;
            }
            _ => break,
        }
    }

    matches
}

fn interpolate_unmatched_segments(
    segments: &mut [TranscriptionSegment],
    matched_by_segment: &[Vec<usize>],
) {
    for index in 0..segments.len() {
        if matched_by_segment
            .get(index)
            .map(|matches| !matches.is_empty())
            .unwrap_or(false)
        {
            continue;
        }

        let previous = (0..index).rev().find(|candidate| {
            matched_by_segment
                .get(*candidate)
                .map(|matches| !matches.is_empty())
                .unwrap_or(false)
        });
        let next = (index + 1..segments.len()).find(|candidate| {
            matched_by_segment
                .get(*candidate)
                .map(|matches| !matches.is_empty())
                .unwrap_or(false)
        });

        match (previous, next) {
            (Some(previous), Some(next))
                if segments[next].start_time > segments[previous].end_time =>
            {
                let gap_start = segments[previous].end_time.saturating_add(DEFAULT_GAP_MS);
                let gap_end = segments[next].start_time.saturating_sub(DEFAULT_GAP_MS);
                let slots = (next - previous) as u64;
                let offset = (index - previous) as u64;
                let span = gap_end.saturating_sub(gap_start);
                let start =
                    gap_start.saturating_add(span.saturating_mul(offset.saturating_sub(1)) / slots);
                let end = gap_start.saturating_add(span.saturating_mul(offset) / slots);
                segments[index].start_time = start;
                segments[index].end_time = end.max(start.saturating_add(MIN_SEGMENT_DURATION_MS));
            }
            (Some(previous), None) => {
                let start = segments[previous].end_time.saturating_add(DEFAULT_GAP_MS);
                segments[index].start_time = start;
                segments[index].end_time = start.saturating_add(
                    estimated_duration_for_text(&segments[index].text).max(MIN_SEGMENT_DURATION_MS),
                );
            }
            (None, Some(next)) => {
                let duration =
                    estimated_duration_for_text(&segments[index].text).max(MIN_SEGMENT_DURATION_MS);
                let end = segments[next].start_time.saturating_sub(DEFAULT_GAP_MS);
                segments[index].end_time = end;
                segments[index].start_time = end.saturating_sub(duration);
            }
            _ => {}
        }
        segments[index].words = aligned_words_for_segment(
            &segments[index].text,
            segments[index].start_time,
            segments[index].end_time,
        );
    }
}

fn normalize_segment_timeline(segments: &mut [TranscriptionSegment]) {
    let mut last_end = 0u64;
    for segment in segments {
        if segment.start_time < last_end {
            segment.start_time = last_end;
        }
        if segment.end_time <= segment.start_time {
            segment.end_time = segment.start_time.saturating_add(MIN_SEGMENT_DURATION_MS);
        }
        last_end = segment.end_time.saturating_add(DEFAULT_GAP_MS.min(40));
        segment.words =
            aligned_words_for_segment(&segment.text, segment.start_time, segment.end_time);
    }
}

fn aligned_words_for_segment(text: &str, start_time: u64, end_time: u64) -> Vec<TranscriptionWord> {
    let display_tokens = display_tokens(text);
    if display_tokens.is_empty() {
        return Vec::new();
    }
    let duration = end_time
        .saturating_sub(start_time)
        .max(display_tokens.len() as u64);
    let total_weight = display_tokens
        .iter()
        .map(|token| token.chars().count().max(1) as u64)
        .sum::<u64>()
        .max(1);
    let mut words = Vec::with_capacity(display_tokens.len());
    let mut current = start_time;
    for (index, token) in display_tokens.iter().enumerate() {
        let end = if index + 1 == display_tokens.len() {
            end_time
        } else {
            current.saturating_add(
                duration.saturating_mul(token.chars().count().max(1) as u64) / total_weight,
            )
        };
        words.push(TranscriptionWord {
            text: token.clone(),
            start_time: current,
            end_time: end.max(current.saturating_add(1)),
        });
        current = end;
    }
    words
}

fn normalized_tokens(text: &str) -> Vec<String> {
    if is_mainly_no_space_language(text) {
        return text
            .chars()
            .filter_map(normalize_char)
            .map(|character| character.to_string())
            .collect();
    }

    let mut tokens = Vec::new();
    let mut current = String::new();
    for character in text.chars().flat_map(char::to_lowercase) {
        if let Some(normalized) = normalize_char(character) {
            current.push(normalized);
        } else if !current.is_empty() {
            tokens.push(std::mem::take(&mut current));
        }
    }
    if !current.is_empty() {
        tokens.push(current);
    }
    tokens
}

fn display_tokens(text: &str) -> Vec<String> {
    if is_mainly_no_space_language(text) {
        return text
            .chars()
            .filter(|character| !character.is_whitespace())
            .map(|character| character.to_string())
            .collect();
    }

    text.split_whitespace()
        .map(str::trim)
        .filter(|token| !token.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

fn normalize_char(character: char) -> Option<char> {
    if let Some(folded) = fold_latin_char(character) {
        Some(folded)
    } else if character.is_ascii_alphanumeric() {
        Some(character.to_ascii_lowercase())
    } else if is_cjk(character) {
        Some(character)
    } else if character.is_alphanumeric() {
        character.to_lowercase().next()
    } else {
        None
    }
}

fn fold_latin_char(character: char) -> Option<char> {
    match character {
        'á' | 'à' | 'â' | 'ä' | 'ã' | 'å' | 'ā' | 'ă' | 'ą' => Some('a'),
        'ç' | 'ć' | 'č' => Some('c'),
        'ď' | 'đ' => Some('d'),
        'é' | 'è' | 'ê' | 'ë' | 'ē' | 'ė' | 'ę' => Some('e'),
        'í' | 'ì' | 'î' | 'ï' | 'ī' | 'į' => Some('i'),
        'ñ' | 'ń' => Some('n'),
        'ó' | 'ò' | 'ô' | 'ö' | 'õ' | 'ō' | 'ő' | 'ø' => Some('o'),
        'ř' | 'ŕ' => Some('r'),
        'ś' | 'š' | 'ş' | 'ș' => Some('s'),
        'ť' | 'ţ' | 'ț' => Some('t'),
        'ú' | 'ù' | 'û' | 'ü' | 'ū' | 'ů' | 'ű' => Some('u'),
        'ý' | 'ÿ' => Some('y'),
        'ž' | 'ź' | 'ż' => Some('z'),
        _ => None,
    }
}

fn is_mainly_no_space_language(text: &str) -> bool {
    let mut cjk = 0usize;
    let mut latin = 0usize;
    for character in text.chars() {
        if is_cjk(character) {
            cjk += 1;
        } else if character.is_ascii_alphabetic() {
            latin += 1;
        }
    }
    cjk > 0 && cjk >= latin
}

fn is_cjk(character: char) -> bool {
    matches!(
        character as u32,
        0x3400..=0x4DBF
            | 0x4E00..=0x9FFF
            | 0xF900..=0xFAFF
            | 0x3040..=0x30FF
            | 0xAC00..=0xD7AF
    )
}

fn estimated_duration_for_text(text: &str) -> u64 {
    let units = display_tokens(text)
        .len()
        .max(text.chars().filter(|ch| !ch.is_whitespace()).count());
    (units as u64)
        .saturating_mul(180)
        .clamp(MIN_SEGMENT_DURATION_MS, 5000)
}

fn ratio(numerator: usize, denominator: usize) -> f64 {
    if denominator == 0 {
        0.0
    } else {
        numerator as f64 / denominator as f64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn segment(
        text: &str,
        start_time: u64,
        end_time: u64,
        words: Vec<TranscriptionWord>,
    ) -> TranscriptionSegment {
        TranscriptionSegment {
            text: text.to_string(),
            start_time,
            end_time,
            uid: String::new(),
            status: String::new(),
            words,
        }
    }

    fn word(text: &str, start_time: u64, end_time: u64) -> TranscriptionWord {
        TranscriptionWord {
            text: text.to_string(),
            start_time,
            end_time,
        }
    }

    #[test]
    fn aligns_english_subtitles_to_asr_words() {
        let downloaded = vec![
            segment("Hello world", 10000, 11000, Vec::new()),
            segment("this is a test", 12000, 13000, Vec::new()),
        ];
        let asr = vec![segment(
            "hello world this is a test",
            0,
            3000,
            vec![
                word("hello", 100, 300),
                word("world", 350, 600),
                word("this", 900, 1100),
                word("is", 1120, 1240),
                word("a", 1260, 1320),
                word("test", 1400, 1700),
            ],
        )];

        let result = align_downloaded_subtitles(&downloaded, &asr).unwrap();
        assert_eq!(result.segments[0].start_time, 100);
        assert_eq!(result.segments[0].end_time, 600);
        assert_eq!(result.segments[1].start_time, 900);
        assert_eq!(result.segments[1].end_time, 1700);
    }

    #[test]
    fn aligns_cjk_subtitles_character_by_character() {
        let downloaded = vec![segment("你好世界", 0, 1000, Vec::new())];
        let asr = vec![segment(
            "你好世界",
            0,
            1000,
            vec![
                word("你", 100, 200),
                word("好", 220, 300),
                word("世", 500, 620),
                word("界", 640, 800),
            ],
        )];

        let result = align_downloaded_subtitles(&downloaded, &asr).unwrap();
        assert_eq!(result.segments[0].start_time, 100);
        assert_eq!(result.segments[0].end_time, 800);
    }

    #[test]
    fn rejects_low_coverage_alignment() {
        let downloaded = vec![segment("completely different text", 0, 1000, Vec::new())];
        let asr = vec![segment(
            "hello world",
            0,
            1000,
            vec![word("hello", 0, 100), word("world", 200, 300)],
        )];

        assert!(align_downloaded_subtitles(&downloaded, &asr).is_err());
    }

    #[test]
    fn aligns_overlapping_youtube_subtitles_to_resegmented_asr() {
        let downloaded = vec![
            segment(
                "We are currently in Kosovo and we are so",
                0,
                6520,
                Vec::new(),
            ),
            segment(
                "excited to try a fast food chain that we",
                3360,
                9600,
                Vec::new(),
            ),
            segment(
                "have never ever heard of. We've seen",
                6520,
                11720,
                Vec::new(),
            ),
            segment(
                "Burger King around, we've seen KFC, and",
                9600,
                13600,
                Vec::new(),
            ),
            segment(
                "we try and eat as much local food as we",
                11720,
                15680,
                Vec::new(),
            ),
            segment(
                "can when we're in a country, but we have",
                13600,
                17960,
                Vec::new(),
            ),
        ];
        let asr = vec![
            segment(
                "we are currently in kosovo",
                160,
                2380,
                vec![
                    word("we", 160, 400),
                    word("are", 430, 650),
                    word("currently", 690, 1260),
                    word("in", 1300, 1440),
                    word("kosovo", 1500, 2380),
                ],
            ),
            segment(
                "And we are so excited to try a fast food chain that we have never heard of",
                2480,
                9050,
                vec![
                    word("And", 2480, 2700),
                    word("we", 2730, 2890),
                    word("are", 2920, 3090),
                    word("so", 3120, 3300),
                    word("excited", 3360, 3840),
                    word("to", 3880, 4020),
                    word("try", 4050, 4300),
                    word("a", 4320, 4400),
                    word("fast", 4450, 4700),
                    word("food", 4750, 5000),
                    word("chain", 5050, 5380),
                    word("that", 5420, 5630),
                    word("we", 5680, 5840),
                    word("have", 5900, 6120),
                    word("never", 6200, 6600),
                    word("heard", 6820, 7200),
                    word("of", 7240, 7400),
                ],
            ),
            segment(
                "We've seen Burger King around. we've seen KFC",
                9050,
                11570,
                vec![
                    word("We've", 9050, 9300),
                    word("seen", 9340, 9650),
                    word("Burger", 9700, 10150),
                    word("King", 10200, 10550),
                    word("around", 10600, 10980),
                    word("we've", 11020, 11240),
                    word("seen", 11280, 11430),
                    word("KFC", 11450, 11570),
                ],
            ),
            segment(
                "And we try to eat as much local food as we can when we're in a country",
                11570,
                15110,
                vec![
                    word("And", 11570, 11740),
                    word("we", 11780, 11920),
                    word("try", 11950, 12140),
                    word("to", 12180, 12300),
                    word("eat", 12350, 12580),
                    word("as", 12620, 12760),
                    word("much", 12800, 13100),
                    word("local", 13140, 13480),
                    word("food", 13520, 13820),
                    word("as", 13860, 14000),
                    word("we", 14030, 14160),
                    word("can", 14200, 14420),
                    word("when", 14460, 14700),
                    word("we're", 14740, 14920),
                    word("in", 14950, 15020),
                    word("a", 15030, 15060),
                    word("country", 15070, 15110),
                ],
            ),
        ];

        let result = align_downloaded_subtitles(&downloaded, &asr).unwrap();
        assert!(result.report.token_coverage > 0.75);
        assert!(result.report.segment_coverage > 0.75);
        assert_eq!(result.segments[0].start_time, 160);
        assert!(result.segments[2].start_time >= 5900);
        assert!(result.segments[5].end_time >= 15110);
    }
}
