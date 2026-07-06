use crate::transcription::TranscriptionSegment;
use serde::Serialize;

const REFERENCE_MATCH_VERSION: &str = "downloaded-reference-correction-v1";
const MIN_TOKEN_COVERAGE: f64 = 0.55;
const MIN_SEGMENT_COVERAGE: f64 = 0.60;
const MAX_ALIGNMENT_MATRIX_CELLS: usize = 25_000_000;
const CHUNK_SUBTITLE_TOKENS: usize = 1_600;
const CHUNK_ASR_PADDING_TOKENS: usize = 800;
const REFERENCE_TIME_PADDING_MS: u64 = 400;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SubtitleReferenceMatchReport {
    pub match_version: String,
    pub token_coverage: f64,
    pub segment_coverage: f64,
    pub matched_token_count: usize,
    pub asr_token_count: usize,
    pub referenced_segment_count: usize,
    pub asr_segment_count: usize,
    pub downloaded_segment_count: usize,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SubtitleReferenceMatch {
    pub asr_index: usize,
    pub reference_text: String,
    pub reference_segment_indices: Vec<usize>,
    pub matched_token_count: usize,
    pub asr_token_count: usize,
    pub confidence: f64,
}

#[derive(Debug, Clone)]
pub struct SubtitleReferenceMatchResult {
    pub matches: Vec<SubtitleReferenceMatch>,
    pub report: SubtitleReferenceMatchReport,
}

#[derive(Debug, Clone)]
struct TimedToken {
    normalized: String,
    segment_index: usize,
}

#[derive(Debug, Clone)]
struct SubtitleToken {
    normalized: String,
    segment_index: usize,
}

pub fn reference_match_version() -> &'static str {
    REFERENCE_MATCH_VERSION
}

pub fn match_downloaded_subtitle_references(
    asr_segments: &[TranscriptionSegment],
    downloaded_segments: &[TranscriptionSegment],
) -> Result<SubtitleReferenceMatchResult, String> {
    if asr_segments.is_empty() {
        return Err("ASR 没有返回可参考字幕".to_string());
    }
    if downloaded_segments.is_empty() {
        return Err("下载字幕没有可用文本".to_string());
    }

    let asr_subtitle_tokens = subtitle_tokens(asr_segments);
    let downloaded_timed_tokens = timed_tokens_for_segments(downloaded_segments);
    if asr_subtitle_tokens.is_empty() {
        return Err("ASR 字幕没有可匹配文本".to_string());
    }
    if downloaded_timed_tokens.is_empty() {
        return Err("下载字幕没有可匹配文本".to_string());
    }

    let matches = monotonic_match(&asr_subtitle_tokens, &downloaded_timed_tokens);
    let matched_count = matches
        .iter()
        .filter(|match_index| match_index.is_some())
        .count();

    let mut referenced_by_asr: Vec<Vec<usize>> = vec![Vec::new(); asr_segments.len()];
    let mut matched_count_by_asr: Vec<usize> = vec![0; asr_segments.len()];
    let mut token_count_by_asr: Vec<usize> = vec![0; asr_segments.len()];
    for (token_index, token) in asr_subtitle_tokens.iter().enumerate() {
        token_count_by_asr[token.segment_index] =
            token_count_by_asr[token.segment_index].saturating_add(1);
        if let Some(downloaded_index) = matches[token_index] {
            matched_count_by_asr[token.segment_index] =
                matched_count_by_asr[token.segment_index].saturating_add(1);
            if let Some(downloaded_token) = downloaded_timed_tokens.get(downloaded_index) {
                referenced_by_asr[token.segment_index].push(downloaded_token.segment_index);
            }
        }
    }

    let mut reference_matches = Vec::with_capacity(asr_segments.len());
    for (asr_index, asr_segment) in asr_segments.iter().enumerate() {
        let mut reference_indices = unique_sorted_indices(&referenced_by_asr[asr_index]);
        if reference_indices.is_empty() {
            reference_indices = subtitle_indices_overlapping_time(downloaded_segments, asr_segment);
        }

        let reference_text = reference_indices
            .iter()
            .filter_map(|index| downloaded_segments.get(*index))
            .map(|segment| segment.text.trim())
            .filter(|text| !text.is_empty())
            .collect::<Vec<_>>()
            .join("\n");

        let asr_token_count = token_count_by_asr[asr_index];
        let matched_token_count = matched_count_by_asr[asr_index];
        reference_matches.push(SubtitleReferenceMatch {
            asr_index,
            reference_text,
            reference_segment_indices: reference_indices,
            matched_token_count,
            asr_token_count,
            confidence: ratio(matched_token_count, asr_token_count),
        });
    }

    let referenced_segment_count = reference_matches
        .iter()
        .filter(|item| !item.reference_text.trim().is_empty())
        .count();
    let token_coverage = ratio(matched_count, asr_subtitle_tokens.len());
    let segment_coverage = ratio(referenced_segment_count, asr_segments.len());
    let mut warnings = Vec::new();
    if token_coverage < MIN_TOKEN_COVERAGE {
        warnings.push(format!(
            "下载字幕参考 token 覆盖率较低（{:.0}%）",
            token_coverage * 100.0
        ));
    }
    if segment_coverage < MIN_SEGMENT_COVERAGE {
        warnings.push(format!(
            "下载字幕参考行覆盖率较低（{:.0}%）",
            segment_coverage * 100.0
        ));
    }

    Ok(SubtitleReferenceMatchResult {
        matches: reference_matches,
        report: SubtitleReferenceMatchReport {
            match_version: REFERENCE_MATCH_VERSION.to_string(),
            token_coverage,
            segment_coverage,
            matched_token_count: matched_count,
            asr_token_count: asr_subtitle_tokens.len(),
            referenced_segment_count,
            asr_segment_count: asr_segments.len(),
            downloaded_segment_count: downloaded_segments.len(),
            warnings,
        },
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

fn timed_tokens_for_segments(segments: &[TranscriptionSegment]) -> Vec<TimedToken> {
    let mut tokens = Vec::new();
    for (segment_index, segment) in segments.iter().enumerate() {
        if segment.words.is_empty() {
            for normalized in normalized_tokens(&segment.text) {
                tokens.push(TimedToken {
                    normalized,
                    segment_index,
                });
            }
        } else {
            for word in &segment.words {
                for normalized in normalized_tokens(&word.text) {
                    tokens.push(TimedToken {
                        normalized,
                        segment_index,
                    });
                }
            }
        }
    }
    tokens
}

fn unique_sorted_indices(indices: &[usize]) -> Vec<usize> {
    let mut result = indices.to_vec();
    result.sort_unstable();
    result.dedup();
    result
}

fn subtitle_indices_overlapping_time(
    downloaded_segments: &[TranscriptionSegment],
    asr_segment: &TranscriptionSegment,
) -> Vec<usize> {
    let start = asr_segment.start_time.saturating_sub(REFERENCE_TIME_PADDING_MS);
    let end = asr_segment.end_time.saturating_add(REFERENCE_TIME_PADDING_MS);
    downloaded_segments
        .iter()
        .enumerate()
        .filter_map(|(index, segment)| {
            if segment.end_time >= start && segment.start_time <= end {
                Some(index)
            } else {
                None
            }
        })
        .collect()
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
    use crate::transcription::TranscriptionWord;

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
    fn matches_downloaded_references_to_asr_segments_without_changing_asr_shape() {
        let asr = vec![segment(
            "hello world",
            0,
            1000,
            vec![word("hello", 100, 300), word("world", 350, 600)],
        )];
        let downloaded = vec![
            segment("Hello world", 5000, 6000, Vec::new()),
            segment("unused text", 6000, 7000, Vec::new()),
        ];

        let result = match_downloaded_subtitle_references(&asr, &downloaded).unwrap();

        assert_eq!(result.matches.len(), 1);
        assert_eq!(result.matches[0].asr_index, 0);
        assert_eq!(result.matches[0].reference_text, "Hello world");
        assert_eq!(result.matches[0].reference_segment_indices, vec![0]);
        assert_eq!(result.report.asr_segment_count, 1);
        assert_eq!(result.report.downloaded_segment_count, 2);
    }

    #[test]
    fn matches_cjk_references_character_by_character() {
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
        let downloaded = vec![segment("你好世界", 0, 1000, Vec::new())];

        let result = match_downloaded_subtitle_references(&asr, &downloaded).unwrap();

        assert_eq!(result.matches[0].reference_text, "你好世界");
        assert_eq!(result.report.token_coverage, 1.0);
    }

    #[test]
    fn keeps_running_with_low_token_coverage_warning() {
        let asr = vec![segment(
            "hello world",
            0,
            1000,
            vec![word("hello", 0, 100), word("world", 200, 300)],
        )];
        let downloaded = vec![segment("completely different text", 0, 1000, Vec::new())];

        let result = match_downloaded_subtitle_references(&asr, &downloaded).unwrap();

        assert!(result.report.token_coverage < MIN_TOKEN_COVERAGE);
        assert!(!result.report.warnings.is_empty());
        assert_eq!(result.matches[0].reference_text, "completely different text");
    }

    #[test]
    fn uses_time_overlap_when_token_match_is_missing() {
        let asr = vec![segment(
            "unmatched asr words",
            1000,
            2000,
            vec![word("unmatched", 1100, 1300)],
        )];
        let downloaded = vec![
            segment("nearby reference", 900, 1800, Vec::new()),
            segment("far away reference", 3000, 4000, Vec::new()),
        ];

        let result = match_downloaded_subtitle_references(&asr, &downloaded).unwrap();

        assert_eq!(result.matches[0].reference_text, "nearby reference");
        assert_eq!(result.matches[0].reference_segment_indices, vec![0]);
    }

    #[test]
    fn matches_overlapping_youtube_subtitles_to_resegmented_asr() {
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

        let result = match_downloaded_subtitle_references(&asr, &downloaded).unwrap();

        assert!(result.report.token_coverage > 0.75);
        assert!(result.report.segment_coverage > 0.75);
        assert_eq!(
            result.matches[0].reference_text,
            "We are currently in Kosovo and we are so"
        );
        assert!(result.matches[1]
            .reference_text
            .contains("excited to try a fast food chain"));
        assert!(result.matches[3]
            .reference_text
            .contains("we try and eat as much local food"));
    }
}
