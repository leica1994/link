use crate::ai::AiService;
use crate::app_log::LogSession;
use crate::settings::AppSettings;
use crate::transcription::{TranscriptionSegment, TranscriptionWord};
use futures::stream::{FuturesUnordered, StreamExt};
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashSet, VecDeque};

const MAX_SEGMENT_WORDS_CJK: usize = 25;
const MAX_SEGMENT_WORDS_ENGLISH: usize = 18;
const MAX_SPLIT_CHUNK_WORDS: usize = 500;
const MAX_SPLIT_ATTEMPTS: usize = 2;
const MAX_CORRECTION_ATTEMPTS: usize = 3;
const RULE_SPLIT_GAP_MS: u64 = 500;
const RULE_MAX_GAP_MS: u64 = 1500;
const TIME_GAP_WINDOW_SIZE: usize = 5;
const TIME_GAP_MULTIPLIER: u64 = 3;
const MIN_TIME_GAP_GROUP_SIZE: usize = 5;
const PREFIX_WORD_RATIO_NUMERATOR: usize = 6;
const PREFIX_WORD_RATIO_DENOMINATOR: usize = 10;
const SUFFIX_WORD_RATIO_NUMERATOR: usize = 4;
const SUFFIX_WORD_RATIO_DENOMINATOR: usize = 10;

#[derive(Debug)]
pub struct SubtitleProcessingResult {
    pub segments: Vec<TranscriptionSegment>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone)]
struct WordUnit {
    text: String,
    start_time: u64,
    end_time: u64,
}

#[derive(Debug, Clone)]
struct CorrectionChunk {
    start_index: usize,
    end_index: usize,
    entries: BTreeMap<String, String>,
}

#[derive(Debug, Clone)]
struct CorrectionChunkResult {
    chunk: CorrectionChunk,
    entries: Vec<(usize, String)>,
}

#[derive(Debug, Clone)]
struct SegmentationBlock {
    block_id: usize,
    original_segments: Vec<TranscriptionSegment>,
    display_segments: Vec<TranscriptionSegment>,
    words: Vec<WordUnit>,
}

trait VideoPromptStrategy {
    fn correction_system_prompt(&self) -> String;
    fn correction_reference(&self) -> &'static str;
    fn split_system_prompt(
        &self,
        max_word_count_cjk: usize,
        max_word_count_english: usize,
    ) -> String;
    fn split_reference(&self) -> &'static str;
}

struct GeneralPromptStrategy;
struct TradingPromptStrategy;

impl VideoPromptStrategy for GeneralPromptStrategy {
    fn correction_system_prompt(&self) -> String {
        r#"你是一位专业字幕校正专家。请在保持原意、语言和段落结构的前提下修正转录字幕。

规则:
1. 只修正识别错误、错别字、明显口癖、非语言声音和标点格式。
2. 不翻译、不改写、不扩写，不替换为同义表达。
3. 保持输入 JSON 的所有 key，不新增、不删除、不合并、不拆分条目。
4. 英文需要修正常规大小写和标点；中文标点保持自然、克制。
5. 输出只能是单个 JSON object，第一字符必须是 {，最后字符必须是 }。
6. 外层只能是字幕 key，key 和 value 都必须使用英文双引号；禁止输出数组、列表、Markdown、代码块、解释或额外文本。"#
            .to_string()
    }

    fn correction_reference(&self) -> &'static str {
        "通用视频内容。优先提升可读性和识别准确性，术语按上下文最小改动修正。"
    }

    fn split_system_prompt(
        &self,
        max_word_count_cjk: usize,
        max_word_count_english: usize,
    ) -> String {
        format!(
            r#"你是一位专业字幕分句专家。请只在自然句界、语义停顿点或强调位置插入 <br>。

规则:
1. 原文必须保持不变，不增删、不改写、不翻译，只插入 <br>。
2. CJK 文本每段不超过 {max_word_count_cjk} 字；拉丁语言每段不超过 {max_word_count_english} 词。
3. 保持每段语义完整，避免过短碎片。
4. 倒计时、关键信息揭示前、转折和强调位置可适当分割。
5. 如果自然句超过长度上限，必须继续在自然停顿点插入 <br>。
6. 直接输出带 <br> 的文本，不要解释或 Markdown。"#
        )
    }

    fn split_reference(&self) -> &'static str {
        "通用视频内容，按自然语义和字幕可读性断句。"
    }
}

impl VideoPromptStrategy for TradingPromptStrategy {
    fn correction_system_prompt(&self) -> String {
        r#"你是一位金融交易视频字幕校正专家，专门处理交易术语、价格行为和市场分析内容。请在保持原意、语言和段落结构的前提下修正转录字幕。

# 基础规则
1. 只修正识别错误、错别字、明显口癖、非语言声音、重复转录错误和标点格式。
2. 不翻译、不改写、不扩写，不改变交易判断、交易方向、风险提示或说话者语气。
3. 保持原始语言：英文输入输出英文，中文输入输出中文，绝不跨语言翻译。
4. 保持输入 JSON 的所有 key，不新增、不删除、不合并、不拆分条目。
5. 输出只能是单个 JSON object，第一字符必须是 {，最后字符必须是 }。
6. 外层只能是字幕 key，key 和 value 都必须使用英文双引号；禁止输出数组、列表、Markdown、代码块、解释或额外文本。
7. 如果上下文不明确，宁可保留原文，也不要猜测或替换为同义表达。

# 交易内容保护规则
1. 严格保留数字、价格、百分比、比例、倍数、杠杆倍数、ticker、币种、货币对、交易所名称、时间周期和方向词。
2. 保持金融记号准确，包括 $、%、x、EUR/USD、BTC/USDT、AAPL、1H、4H、1D、1W、1M 等。
3. 标准化技术指标和平台名称：RSI、MACD、MA、EMA、SMA、Bollinger Bands、TradingView、MetaTrader、Interactive Brokers。
4. 保留常见交易讲师和人名：Ali Moin Afshari、Al Brooks、Rose、Tim Fairweather、Tom Hougaard、Richard。
5. 保持交易社区常用表达和术语，不要把 long/short、bullish/bearish、support/resistance、bar、setup 等改成非交易语境表达。

# 重点交易术语
优先保护并按上下文修正常见价格行为词汇：
price action, trading setup, pullback, trend, breakout, reversal, channel, wedge, flag, support, resistance, swing high, swing low, bar pattern, signal, entry, exit, stop loss, profit target, always in long, always in short, scalp, context, premise, reasonable trade, management, risk reward, measured move, leg, spike, climax, exhaustion, gap, momentum, continuation, correction, inside bar, outside bar, pin bar, doji, hammer, shooting star, engulfing, bull trap, bear trap, failed breakout, double top, double bottom, head and shoulders, EMA, moving average, confluence zone, supply, demand, order flow, limit order, stop order, market order, position sizing, trend line, horizontal line, diagonal, strong, weak, pressure, vacuum, magnet, first pullback, second entry, high probability, micro double top, micro double bottom, micro E-mini, major trend, minor pullback, trading range, day structure, trading range day structure, day type, micro channel, micro gap, broad channel, tight channel, tight trading range, context transition, MTR, major trend reversal, follow through, Pro Trader Mentoring, I1R, pause bar, swing trade, countertrend scalp, countertrend, fade, strong bulls and bears, scratch, trap, trapped in a trade, trapped out of a trade, failed failure, second signal, double bottom bull flag, three push, wedge flag, barb wire, spike and channel, buying pressure, pressing their longs, pressing their shorts, micro measuring gap, 20 moving average gap bars, moving average gap bar, gap bar, second moving average gap bar setup, sell the close, buy the close。

# 强制修正规则
1. Al Brooks 价格行为语境中不存在 "macro channel" 或 "macro gap"。
2. 如果 ASR 输出 "macro channel" / "macro gap"（包括大小写、连字符、复数等变体），必须修正为 "micro channel" / "micro gap"，绝不能保留 macro channel / macro gap。
3. "macro E-mini" 必须修正为 "micro E-mini"。
4. 根据上下文修正交易术语同音误识别：dog/dogy/dodgy→doji，mack d/mac d→MACD，are s i→RSI，e may→EMA，bowling/bollinger→Bollinger Bands。
5. 仅在交易上下文清楚时修正误识别；除 macro→micro 的强制规则外，不确定时保留原 token。

# 输出格式
返回与输入 key 完全一致的 JSON 对象：
{
  "1": "校正后的字幕",
  "2": "校正后的字幕"
}"#
            .to_string()
    }

    fn correction_reference(&self) -> &'static str {
        "交易视频内容。重点保护价格、比例、方向、周期、币种、ticker、技术指标、价格行为术语和 Al Brooks 交易术语，不擅自改变结论。"
    }

    fn split_system_prompt(
        &self,
        max_word_count_cjk: usize,
        max_word_count_english: usize,
    ) -> String {
        format!(
            r#"你是一位专业交易视频字幕分句专家。请只在自然句界、语义停顿点或交易信息边界插入 <br>。

规则:
1. 原文必须保持不变，不增删、不改写、不翻译，只插入 <br>。
2. CJK 文本每段不超过 {max_word_count_cjk} 字；拉丁语言每段不超过 {max_word_count_english} 词。
3. 价格、百分比、杠杆、ticker、币种、K线周期、做多/做空、止损/止盈等关键信息不要拆散。
4. 在交易观点切换、条件触发、风险提示、入场/出场说明处优先分割。
5. 如果自然句超过长度上限，必须继续在交易信息边界插入 <br>。
6. 直接输出带 <br> 的文本，不要解释或 Markdown。"#
        )
    }

    fn split_reference(&self) -> &'static str {
        "交易视频内容，断句时保护交易术语、数字和条件关系。"
    }
}

pub async fn smart_segment_subtitles<F>(
    settings: &AppSettings,
    ai_service: &AiService,
    log_session: &LogSession,
    segments: Vec<TranscriptionSegment>,
    report: &mut F,
) -> SubtitleProcessingResult
where
    F: FnMut(u8, &str, &[TranscriptionSegment], &[String]),
{
    if segments.is_empty() {
        return SubtitleProcessingResult {
            segments,
            warnings: Vec::new(),
        };
    }

    let mut blocks = build_segmentation_blocks(&segments);
    if blocks.is_empty() || blocks.iter().all(|block| block.words.is_empty()) {
        return SubtitleProcessingResult {
            segments,
            warnings: Vec::new(),
        };
    }

    let strategy = prompt_strategy_for(&settings.video_content_type);
    let system_prompt =
        strategy.split_system_prompt(MAX_SEGMENT_WORDS_CJK, MAX_SEGMENT_WORDS_ENGLISH);
    let reference = strategy.split_reference().to_string();
    let skipped_blocks = mark_blocks_without_split_needed(&mut blocks);
    let mut failed_blocks = 0usize;
    let mut warnings = Vec::new();

    log_session.info(
        "smart_segmentation_stage_prepared",
        "AI 智能断句批次已准备",
        json!({
            "inputSegmentCount": segments.len(),
            "blockCount": blocks.len(),
            "maxSplitChunkWords": MAX_SPLIT_CHUNK_WORDS,
            "skippedBlockCount": skipped_blocks,
            "videoContentType": &settings.video_content_type,
            "llmMode": "configured_llm_settings",
        }),
    );

    let total = blocks.len().max(1);
    let max_active = active_ai_work_count(settings);
    let mut split_futures = FuturesUnordered::new();
    let mut next_block_index = 0usize;
    while next_block_index < blocks.len() && split_futures.len() < max_active {
        if !block_needs_split_ai(&blocks[next_block_index]) {
            next_block_index += 1;
            continue;
        }

        let block = &mut blocks[next_block_index];
        set_segments_status(&mut block.display_segments, "segmenting");

        let block_id = block.block_id;
        let words = block.words.clone();
        let block_system_prompt = system_prompt.clone();
        let block_reference = reference.clone();
        let block_log_session = log_session.clone();

        split_futures.push(run_split_block(
            settings,
            ai_service,
            block_id,
            block_system_prompt,
            block_reference,
            words,
            block_log_session,
        ));
        next_block_index += 1;
    }

    let mut completed = skipped_blocks;
    let snapshot = render_segmentation_blocks(&blocks);
    report(
        stage_progress(0, 100, completed, total),
        "AI 智能断句中",
        &snapshot,
        &warnings,
    );

    if split_futures.is_empty() {
        return SubtitleProcessingResult {
            segments: render_segmentation_blocks(&blocks),
            warnings,
        };
    }

    while let Some((block_id, result)) = split_futures.next().await {
        completed += 1;

        if let Some(block) = blocks.iter_mut().find(|block| block.block_id == block_id) {
            match result {
                Ok(mut processed_segments) if !processed_segments.is_empty() => {
                    assign_segment_metadata(
                        &mut processed_segments,
                        &format!("seg-{block_id}"),
                        "segmented",
                    );
                    block.display_segments = processed_segments;
                }
                Ok(_) => {
                    block.display_segments = block.original_segments.clone();
                    set_segments_status(&mut block.display_segments, "kept");
                    failed_blocks += 1;
                    log_session.warn(
                        "smart_segmentation_block_empty",
                        "智能断句批次未返回可用结果，已保留原文",
                        json!({
                            "blockIndex": block_id + 1,
                            "originalSegmentCount": block.original_segments.len(),
                        }),
                    );
                }
                Err(error) => {
                    block.display_segments = block.original_segments.clone();
                    set_segments_status(&mut block.display_segments, "kept");
                    failed_blocks += 1;
                    log_session.warn(
                        "smart_segmentation_block_failed",
                        "智能断句批次失败，已保留原文",
                        json!({
                            "blockIndex": block_id + 1,
                            "originalSegmentCount": block.original_segments.len(),
                            "error": &error,
                        }),
                    );
                }
            }
        }

        while next_block_index < blocks.len() && split_futures.len() < max_active {
            if !block_needs_split_ai(&blocks[next_block_index]) {
                next_block_index += 1;
                continue;
            }

            let block = &mut blocks[next_block_index];
            set_segments_status(&mut block.display_segments, "segmenting");

            let block_id = block.block_id;
            let words = block.words.clone();
            let block_system_prompt = system_prompt.clone();
            let block_reference = reference.clone();
            let block_log_session = log_session.clone();

            split_futures.push(run_split_block(
                settings,
                ai_service,
                block_id,
                block_system_prompt,
                block_reference,
                words,
                block_log_session,
            ));
            next_block_index += 1;
        }

        warnings = build_processing_warnings("智能断句", failed_blocks, "断句批次");
        let snapshot = render_segmentation_blocks(&blocks);
        let progress = stage_progress(0, 100, completed, total);
        let message = if completed == total {
            "智能断句完成"
        } else {
            "智能断句中"
        };
        report(progress, message, &snapshot, &warnings);
    }

    let processed_segments = render_segmentation_blocks(&blocks);
    if failed_blocks > 0 {
        log_session.warn(
            "smart_segmentation_stage_partial",
            "AI 智能断句部分批次失败，已保留原文",
            json!({
                "failedBlockCount": failed_blocks,
                "blockCount": total,
            }),
        );
    }

    SubtitleProcessingResult {
        segments: processed_segments,
        warnings,
    }
}

pub async fn correct_subtitles<F>(
    settings: &AppSettings,
    ai_service: &AiService,
    log_session: &LogSession,
    segments: Vec<TranscriptionSegment>,
    report: &mut F,
) -> SubtitleProcessingResult
where
    F: FnMut(u8, &str, &[TranscriptionSegment], &[String]),
{
    if segments.is_empty() {
        return SubtitleProcessingResult {
            segments,
            warnings: Vec::new(),
        };
    }

    let strategy = prompt_strategy_for(&settings.video_content_type);
    let system_prompt = strategy.correction_system_prompt();
    let reference = strategy.correction_reference().to_string();
    let chunks =
        build_correction_chunks(&segments, settings.translation_batch_size.max(1) as usize);
    let mut corrected_segments = segments;
    let mut failed_chunks = 0usize;
    let mut warnings = Vec::new();

    if chunks.is_empty() {
        return SubtitleProcessingResult {
            segments: corrected_segments,
            warnings,
        };
    }

    log_session.info(
        "subtitle_correction_stage_prepared",
        "AI 字幕校正批次已准备",
        json!({
            "inputSegmentCount": corrected_segments.len(),
            "chunkCount": chunks.len(),
            "batchSize": settings.translation_batch_size.max(1),
            "videoContentType": &settings.video_content_type,
            "llmMode": "configured_llm_settings_json_response",
        }),
    );

    let total = chunks.len().max(1);
    let max_active = active_ai_work_count(settings);
    let mut correction_futures = FuturesUnordered::new();
    let mut next_chunk_index = 0usize;
    while next_chunk_index < chunks.len() && correction_futures.len() < max_active {
        let chunk = chunks[next_chunk_index].clone();
        mark_range_status(
            &mut corrected_segments,
            chunk.start_index,
            chunk.end_index,
            "correcting",
        );
        let chunk_system_prompt = system_prompt.clone();
        let chunk_reference = reference.clone();
        let chunk_log_session = log_session.clone();
        correction_futures.push(run_correction_chunk(
            settings,
            ai_service,
            chunk_system_prompt,
            chunk_reference,
            chunk,
            chunk_log_session,
        ));
        next_chunk_index += 1;
    }
    report(0, "AI 字幕校正中", &corrected_segments, &warnings);

    let mut completed = 0usize;

    while let Some(result) = correction_futures.next().await {
        completed += 1;

        match result {
            Ok(result) => {
                for (index, text) in result.entries {
                    if let Some(segment) = corrected_segments.get_mut(index) {
                        segment.text = text;
                    }
                }
                mark_range_status(
                    &mut corrected_segments,
                    result.chunk.start_index,
                    result.chunk.end_index,
                    "corrected",
                );
            }
            Err((chunk, error)) => {
                mark_range_status(
                    &mut corrected_segments,
                    chunk.start_index,
                    chunk.end_index,
                    "kept",
                );
                failed_chunks += 1;
                log_session.warn(
                    "subtitle_correction_chunk_failed",
                    "字幕校正批次失败，已保留原文",
                    json!({
                        "startIndex": chunk.start_index + 1,
                        "endIndex": chunk.end_index + 1,
                        "entryCount": chunk.entries.len(),
                        "error": &error,
                    }),
                );
            }
        }

        while next_chunk_index < chunks.len() && correction_futures.len() < max_active {
            let chunk = chunks[next_chunk_index].clone();
            mark_range_status(
                &mut corrected_segments,
                chunk.start_index,
                chunk.end_index,
                "correcting",
            );
            let chunk_system_prompt = system_prompt.clone();
            let chunk_reference = reference.clone();
            let chunk_log_session = log_session.clone();
            correction_futures.push(run_correction_chunk(
                settings,
                ai_service,
                chunk_system_prompt,
                chunk_reference,
                chunk,
                chunk_log_session,
            ));
            next_chunk_index += 1;
        }

        warnings = build_processing_warnings("字幕校正", failed_chunks, "校正批次");
        let progress = stage_progress(0, 100, completed, total);
        let message = if completed == total {
            "字幕校正完成"
        } else {
            "字幕校正中"
        };
        report(progress, message, &corrected_segments, &warnings);
    }

    if failed_chunks > 0 {
        log_session.warn(
            "subtitle_correction_stage_partial",
            "AI 字幕校正部分批次失败，已保留原文",
            json!({
                "failedChunkCount": failed_chunks,
                "chunkCount": total,
            }),
        );
    }

    SubtitleProcessingResult {
        segments: corrected_segments,
        warnings,
    }
}

fn build_segmentation_blocks(segments: &[TranscriptionSegment]) -> Vec<SegmentationBlock> {
    let mut blocks = Vec::new();
    let mut current_segments = Vec::new();
    let mut current_words = Vec::new();
    let mut current_word_count = 0usize;

    for segment in segments {
        let segment_words = build_word_units(std::slice::from_ref(segment));
        let segment_word_count = count_word_units(&segment_words);

        if !current_segments.is_empty()
            && current_word_count + segment_word_count > MAX_SPLIT_CHUNK_WORDS
        {
            push_segmentation_block(&mut blocks, &mut current_segments, &mut current_words);
            current_word_count = 0;
        }

        current_segments.push(segment.clone());
        current_words.extend(segment_words);
        current_word_count += segment_word_count;
    }

    push_segmentation_block(&mut blocks, &mut current_segments, &mut current_words);
    blocks
}

fn push_segmentation_block(
    blocks: &mut Vec<SegmentationBlock>,
    current_segments: &mut Vec<TranscriptionSegment>,
    current_words: &mut Vec<WordUnit>,
) {
    if current_segments.is_empty() {
        return;
    }

    let block_id = blocks.len();
    let original_segments = std::mem::take(current_segments);
    let display_segments = original_segments.clone();
    let words = std::mem::take(current_words);

    blocks.push(SegmentationBlock {
        block_id,
        original_segments,
        display_segments,
        words,
    });
}

fn render_segmentation_blocks(blocks: &[SegmentationBlock]) -> Vec<TranscriptionSegment> {
    blocks
        .iter()
        .flat_map(|block| block.display_segments.iter().cloned())
        .collect()
}

fn mark_blocks_without_split_needed(blocks: &mut [SegmentationBlock]) -> usize {
    let mut skipped = 0usize;

    for block in blocks {
        if block_needs_split_ai(block) {
            continue;
        }

        set_segments_status(&mut block.display_segments, "segmented");
        skipped += 1;
    }

    skipped
}

fn block_needs_split_ai(block: &SegmentationBlock) -> bool {
    if block.words.is_empty() {
        return false;
    }

    if block.original_segments.len() != 1 {
        return true;
    }

    let text = block
        .original_segments
        .first()
        .map(|segment| segment.text.as_str())
        .unwrap_or_default();

    count_words(text) > max_segment_words_for_text(text)
}

fn assign_segment_metadata(segments: &mut [TranscriptionSegment], uid_prefix: &str, status: &str) {
    for (index, segment) in segments.iter_mut().enumerate() {
        segment.uid = format!("{uid_prefix}-{index}");
        segment.status = status.to_string();
    }
}

fn set_segments_status(segments: &mut [TranscriptionSegment], status: &str) {
    for segment in segments {
        segment.status = status.to_string();
    }
}

fn mark_range_status(
    segments: &mut [TranscriptionSegment],
    start_index: usize,
    end_index: usize,
    status: &str,
) {
    if start_index >= segments.len() {
        return;
    }

    let end_index = end_index.min(segments.len().saturating_sub(1));
    for segment in &mut segments[start_index..=end_index] {
        segment.status = status.to_string();
    }
}

fn count_word_units(words: &[WordUnit]) -> usize {
    words
        .iter()
        .map(|word| count_words(&word.text).max(1))
        .sum::<usize>()
}

fn stage_progress(start: u8, end: u8, completed: usize, total: usize) -> u8 {
    let span = end.saturating_sub(start) as usize;
    let scaled = if total == 0 {
        span
    } else {
        span.saturating_mul(completed) / total
    };
    start.saturating_add(scaled as u8).min(end)
}

fn build_processing_warnings(stage: &str, failed_count: usize, unit_name: &str) -> Vec<String> {
    if failed_count == 0 {
        Vec::new()
    } else {
        vec![format!(
            "{stage}部分失败，已保留 {failed_count} 个{unit_name}的原文，详情已写入日志"
        )]
    }
}

fn active_ai_work_count(settings: &AppSettings) -> usize {
    settings.translation_thread_count.max(1) as usize
}

async fn run_split_block(
    settings: &AppSettings,
    ai_service: &AiService,
    block_id: usize,
    system_prompt: String,
    reference: String,
    words: Vec<WordUnit>,
    log_session: LogSession,
) -> (usize, Result<Vec<TranscriptionSegment>, String>) {
    (
        block_id,
        split_chunk_by_llm(
            settings,
            ai_service,
            system_prompt,
            reference,
            words,
            log_session,
        )
        .await,
    )
}

async fn run_correction_chunk(
    settings: &AppSettings,
    ai_service: &AiService,
    system_prompt: String,
    reference: String,
    chunk: CorrectionChunk,
    log_session: LogSession,
) -> Result<CorrectionChunkResult, (CorrectionChunk, String)> {
    correct_chunk_by_llm(
        settings,
        ai_service,
        system_prompt,
        reference,
        chunk,
        log_session,
    )
    .await
}

async fn split_chunk_by_llm(
    settings: &AppSettings,
    ai_service: &AiService,
    system_prompt: String,
    reference: String,
    words: Vec<WordUnit>,
    log_session: LogSession,
) -> Result<Vec<TranscriptionSegment>, String> {
    let source_text = join_word_units(&words);
    let mut feedback = String::new();
    let max_output_tokens = estimate_max_output_tokens(&source_text);

    for attempt in 1..=MAX_SPLIT_ATTEMPTS {
        let user_prompt = build_split_user_prompt(&source_text, &reference, &feedback);
        let response = match ai_service
            .chat(
                settings,
                system_prompt.clone(),
                user_prompt,
                max_output_tokens,
            )
            .await
        {
            Ok(response) => response,
            Err(error) => {
                log_session.warn(
                    "smart_segmentation_llm_request_failed",
                    "智能断句 LLM 请求失败",
                    json!({
                        "attempt": attempt,
                        "error": &error,
                    }),
                );
                return Err(error);
            }
        };
        let sentences = parse_split_response(&response);
        let original_sentence_count = sentences.len();

        match normalize_split_sentences_by_rules(&source_text, &words, &sentences).and_then(
            |sentences| validate_split_result(&source_text, &sentences).map(|()| sentences),
        ) {
            Ok(sentences) => {
                if sentences.len() != original_sentence_count {
                    log_session.info(
                        "smart_segmentation_rule_repaired",
                        "智能断句结果已按规则补充分割",
                        json!({
                            "attempt": attempt,
                            "sourceChars": source_text.chars().count(),
                            "sentenceCount": sentences.len(),
                        }),
                    );
                }
                return Ok(merge_word_units_by_sentences(&words, &sentences));
            }
            Err(error) => {
                log_session.warn(
                    "smart_segmentation_validation_failed",
                    "智能断句结果校验失败，准备重试",
                    json!({
                        "attempt": attempt,
                        "sourceChars": source_text.chars().count(),
                        "sentenceCount": sentences.len(),
                        "error": &error,
                    }),
                );
                feedback = format!(
                    "上一次结果无效: {error}\n请重新输出完整文本，只允许插入 <br>，每段必须满足长度上限，不要解释。"
                );
            }
        }
    }

    let fallback_sentences = rule_split_word_units(&words);
    match validate_split_result(&source_text, &fallback_sentences) {
        Ok(()) => {
            log_session.warn(
                "smart_segmentation_rule_fallback_used",
                "智能断句 LLM 多次校验失败，已使用规则断句兜底",
                json!({
                    "sourceChars": source_text.chars().count(),
                    "sentenceCount": fallback_sentences.len(),
                }),
            );
            Ok(merge_word_units_by_sentences(&words, &fallback_sentences))
        }
        Err(error) => {
            log_session.warn(
                "smart_segmentation_rule_fallback_failed",
                "智能断句规则兜底失败",
                json!({
                    "sourceChars": source_text.chars().count(),
                    "sentenceCount": fallback_sentences.len(),
                    "error": &error,
                }),
            );
            Err("LLM 断句结果多次校验失败".to_string())
        }
    }
}

async fn correct_chunk_by_llm(
    settings: &AppSettings,
    ai_service: &AiService,
    system_prompt: String,
    reference: String,
    chunk: CorrectionChunk,
    log_session: LogSession,
) -> Result<CorrectionChunkResult, (CorrectionChunk, String)> {
    let max_output_tokens = estimate_max_output_tokens(
        &chunk
            .entries
            .values()
            .cloned()
            .collect::<Vec<_>>()
            .join("\n"),
    );
    let mut feedback = String::new();

    for attempt in 1..=MAX_CORRECTION_ATTEMPTS {
        let user_prompt = build_correction_user_prompt(&chunk.entries, &reference, &feedback);
        let response = match ai_service
            .chat_for_json_output(
                settings,
                system_prompt.clone(),
                user_prompt,
                max_output_tokens,
            )
            .await
        {
            Ok(response) => response,
            Err(error) => {
                log_session.warn(
                    "subtitle_correction_llm_request_failed",
                    "字幕校正 LLM 请求失败",
                    json!({
                        "attempt": attempt,
                        "startIndex": chunk.start_index + 1,
                        "endIndex": chunk.end_index + 1,
                        "error": &error,
                    }),
                );
                return Err((chunk, error));
            }
        };
        let parsed = match parse_json_text_map(&response) {
            Ok(parsed) => parsed,
            Err(error) => {
                feedback = build_correction_json_feedback(&chunk.entries, &error);
                continue;
            }
        };

        match validate_correction_keys(&chunk.entries, &parsed) {
            Ok(()) => {
                let entries = parsed
                    .into_iter()
                    .filter_map(|(key, text)| {
                        key.parse::<usize>().ok().map(|index| (index - 1, text))
                    })
                    .collect();
                return Ok(CorrectionChunkResult { chunk, entries });
            }
            Err(error) => {
                feedback = build_correction_key_feedback(&chunk.entries, &error);
            }
        }
    }

    Err((chunk, "LLM 校正结果多次校验失败".to_string()))
}

fn prompt_strategy_for(video_content_type: &str) -> Box<dyn VideoPromptStrategy + Send + Sync> {
    match video_content_type {
        "trading" => Box::new(TradingPromptStrategy),
        _ => Box::new(GeneralPromptStrategy),
    }
}

fn build_split_user_prompt(source_text: &str, reference: &str, feedback: &str) -> String {
    let mut prompt = format!(
        "请用 <br> 分隔以下字幕文本。\n<reference>{reference}</reference>\n<input>{source_text}</input>"
    );

    if !feedback.is_empty() {
        prompt.push_str("\n<feedback>");
        prompt.push_str(feedback);
        prompt.push_str("</feedback>");
    }

    prompt
}

fn build_correction_user_prompt(
    entries: &BTreeMap<String, String>,
    reference: &str,
    feedback: &str,
) -> String {
    let input_json = serde_json::to_string(entries).unwrap_or_else(|_| "{}".to_string());
    let input_lines = entries
        .iter()
        .map(|(key, text)| format!("{key}\t{text}"))
        .collect::<Vec<_>>()
        .join("\n");
    let mut prompt = format!(
        "请校正以下字幕。保持原始语言，不要翻译。\n\
         输入格式是每行一个字幕 key 和原文，中间用 Tab 分隔；最终必须输出一个 JSON 对象，key 必须与输入完全一致，value 是校正后的字幕文本。\n\
         可以思考，但最终答案只能是 JSON 对象；不要在最终答案中复述规则、输入或分析过程。\n\
         <reference>{reference}</reference>\n\
         <input_subtitle>\n{input_lines}\n</input_subtitle>\n\
         <output_template>{input_json}</output_template>\n\
         <template_rule>最终答案必须复制 output_template 的完整 JSON object 外层结构和全部 key，只改 value 内容；不需要校正的 value 原样保留。</template_rule>\n\
         <final_answer_rule>最终答案第一字符必须是 {{，最后字符必须是 }}，且必须能被 JSON.parse 直接解析。</final_answer_rule>"
    );

    if !feedback.is_empty() {
        prompt.push_str("\n<feedback>");
        prompt.push_str(feedback);
        prompt.push_str("</feedback>");
    }

    prompt
}

fn build_correction_json_feedback(entries: &BTreeMap<String, String>, error: &str) -> String {
    let output_template = serde_json::to_string(entries).unwrap_or_else(|_| "{}".to_string());

    format!(
        "上一次结果不是有效 JSON: {error}\n请只输出完整 JSON 对象，第一字符必须是 {{，最后字符必须是 }}。key 和 value 都必须使用英文双引号。请复制这个 JSON object 的外层结构，只改 value: {output_template}"
    )
}

fn build_correction_key_feedback(entries: &BTreeMap<String, String>, error: &str) -> String {
    let output_template = serde_json::to_string(entries).unwrap_or_else(|_| "{}".to_string());

    format!(
        "上一次结果 key 不匹配: {error}\n请输出完整 JSON，必须包含原始所有 key。请复制这个 JSON object 的外层结构，只改 value: {output_template}"
    )
}

fn build_word_units(segments: &[TranscriptionSegment]) -> Vec<WordUnit> {
    segments
        .iter()
        .flat_map(|segment| {
            if segment.words.is_empty() {
                estimate_word_units(segment)
            } else {
                let words = segment
                    .words
                    .iter()
                    .filter_map(|word| {
                        let text = word.text.trim().to_string();
                        if text.is_empty() {
                            None
                        } else {
                            Some(WordUnit {
                                text,
                                start_time: word.start_time,
                                end_time: word.end_time,
                            })
                        }
                    })
                    .collect::<Vec<_>>();

                if words.is_empty() {
                    estimate_word_units(segment)
                } else {
                    words
                }
            }
        })
        .collect()
}

fn estimate_word_units(segment: &TranscriptionSegment) -> Vec<WordUnit> {
    let tokens = split_text_tokens(&segment.text);
    if tokens.is_empty() {
        return Vec::new();
    }

    let duration = segment.end_time.saturating_sub(segment.start_time);
    let total_weight = tokens
        .iter()
        .map(|token| normalized_len(token).max(1) as u64)
        .sum::<u64>()
        .max(1);
    let mut current_time = segment.start_time;
    let mut units = Vec::with_capacity(tokens.len());

    for (index, token) in tokens.iter().enumerate() {
        let weight = normalized_len(token).max(1) as u64;
        let end_time = if index == tokens.len() - 1 {
            segment.end_time
        } else {
            current_time.saturating_add(duration.saturating_mul(weight) / total_weight)
        };

        units.push(WordUnit {
            text: token.clone(),
            start_time: current_time,
            end_time,
        });
        current_time = end_time;
    }

    units
}

fn split_text_tokens(text: &str) -> Vec<String> {
    if is_mainly_no_space_language(text) {
        return text
            .chars()
            .filter(|character| !character.is_whitespace())
            .map(|character| character.to_string())
            .collect();
    }

    text.split_whitespace().map(ToOwned::to_owned).collect()
}

fn build_correction_chunks(
    segments: &[TranscriptionSegment],
    batch_size: usize,
) -> Vec<CorrectionChunk> {
    segments
        .chunks(batch_size.max(1))
        .enumerate()
        .map(|(chunk_index, chunk)| {
            let start_index = chunk_index * batch_size.max(1);
            let mut entries = BTreeMap::new();

            for (offset, segment) in chunk.iter().enumerate() {
                let index = start_index + offset;
                entries.insert((index + 1).to_string(), segment.text.clone());
            }

            CorrectionChunk {
                start_index,
                end_index: start_index + chunk.len().saturating_sub(1),
                entries,
            }
        })
        .collect()
}

fn join_word_units(words: &[WordUnit]) -> String {
    let compact_text = words
        .iter()
        .map(|word| word.text.as_str())
        .collect::<String>();
    if is_mainly_no_space_language(&compact_text) {
        compact_text
    } else {
        words
            .iter()
            .map(|word| word.text.trim())
            .filter(|text| !text.is_empty())
            .collect::<Vec<_>>()
            .join(" ")
    }
}

fn parse_split_response(response: &str) -> Vec<String> {
    response
        .replace('\n', "")
        .split("<br>")
        .map(str::trim)
        .filter(|segment| !segment.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

fn normalize_split_sentences_by_rules(
    source_text: &str,
    words: &[WordUnit],
    sentences: &[String],
) -> Result<Vec<String>, String> {
    if sentences.is_empty() {
        return Err("没有找到 <br> 分段结果".to_string());
    }

    let source_normalized = normalize_content(source_text);
    let merged_normalized = normalize_content(&sentences.join(""));
    if source_normalized != merged_normalized {
        return Err("断句结果修改了原文内容".to_string());
    }

    let max_allowed = max_segment_words_for_text(source_text);
    let mut normalized = Vec::new();
    let mut word_index = 0usize;
    for sentence in sentences {
        let target_len = normalized_len(sentence);
        if target_len == 0 {
            continue;
        }

        let start_index = word_index;
        let mut current_len = 0usize;
        while word_index < words.len() && (current_len < target_len || word_index == start_index) {
            current_len += normalized_len(&words[word_index].text).max(1);
            word_index += 1;
        }

        if start_index >= word_index {
            continue;
        }

        let group = &words[start_index..word_index];
        let time_groups = group_word_units_by_time_gaps(group, RULE_MAX_GAP_MS, false);
        for time_group in time_groups {
            let text = join_word_units(&time_group);
            if count_words(&text) > max_allowed {
                normalized.extend(rule_split_word_units(&time_group));
            } else {
                normalized.push(text);
            }
        }
    }

    if word_index < words.len() {
        normalized.extend(rule_split_word_units(&words[word_index..]));
    }

    Ok(normalized)
}

fn validate_split_result(source_text: &str, sentences: &[String]) -> Result<(), String> {
    if sentences.is_empty() {
        return Err("没有找到 <br> 分段结果".to_string());
    }

    let source_normalized = normalize_content(source_text);
    let merged_normalized = normalize_content(&sentences.join(""));
    if source_normalized != merged_normalized {
        return Err("断句结果修改了原文内容".to_string());
    }

    let max_allowed = max_segment_words_for_text(source_text);

    if let Some((index, count)) = sentences
        .iter()
        .enumerate()
        .map(|(index, sentence)| (index, count_words(sentence)))
        .find(|(_, count)| *count > max_allowed)
    {
        return Err(format!(
            "第 {} 段过长（{} > {}）",
            index + 1,
            count,
            max_allowed
        ));
    }

    Ok(())
}

fn rule_split_word_units(words: &[WordUnit]) -> Vec<String> {
    let mut result = Vec::new();
    for group in group_word_units_by_time_gaps(words, RULE_SPLIT_GAP_MS, true) {
        for common_group in split_word_units_by_common_words(&group) {
            result.extend(split_long_word_units(&common_group));
        }
    }

    result
        .into_iter()
        .map(|text| text.trim().to_string())
        .filter(|text| !text.is_empty())
        .collect()
}

fn group_word_units_by_time_gaps(
    words: &[WordUnit],
    max_gap_ms: u64,
    check_large_gaps: bool,
) -> Vec<Vec<WordUnit>> {
    if words.is_empty() {
        return Vec::new();
    }

    let mut result = Vec::new();
    let mut current_group = vec![words[0].clone()];
    let mut recent_gaps: VecDeque<u64> = VecDeque::new();

    for index in 1..words.len() {
        let time_gap = words[index]
            .start_time
            .saturating_sub(words[index - 1].end_time);
        let mut should_split = time_gap > max_gap_ms;

        if check_large_gaps {
            recent_gaps.push_back(time_gap);
            if recent_gaps.len() > TIME_GAP_WINDOW_SIZE {
                recent_gaps.pop_front();
            }
            if recent_gaps.len() == TIME_GAP_WINDOW_SIZE {
                let gap_sum = recent_gaps.iter().sum::<u64>();
                let average_gap = gap_sum / TIME_GAP_WINDOW_SIZE as u64;
                if average_gap > 0
                    && time_gap > average_gap.saturating_mul(TIME_GAP_MULTIPLIER)
                    && current_group.len() > MIN_TIME_GAP_GROUP_SIZE
                {
                    should_split = true;
                }
            }
        }

        if should_split {
            result.push(std::mem::take(&mut current_group));
            recent_gaps.clear();
        }

        current_group.push(words[index].clone());
    }

    if !current_group.is_empty() {
        result.push(current_group);
    }

    result
}

fn split_word_units_by_common_words(words: &[WordUnit]) -> Vec<Vec<WordUnit>> {
    if words.is_empty() {
        return Vec::new();
    }

    let mut result = Vec::new();
    let mut current_group = Vec::new();

    for (index, word) in words.iter().enumerate() {
        let max_word_count = max_segment_words_for_text(&word.text);
        let prefix_threshold = ratio_threshold(
            max_word_count,
            PREFIX_WORD_RATIO_NUMERATOR,
            PREFIX_WORD_RATIO_DENOMINATOR,
        );
        let suffix_threshold = ratio_threshold(
            max_word_count,
            SUFFIX_WORD_RATIO_NUMERATOR,
            SUFFIX_WORD_RATIO_DENOMINATOR,
        );

        if !current_group.is_empty()
            && count_word_units(&current_group) >= prefix_threshold
            && is_prefix_split_word(&word.text)
        {
            result.push(std::mem::take(&mut current_group));
        }

        if index > 0
            && !current_group.is_empty()
            && count_word_units(&current_group) >= suffix_threshold
            && is_suffix_split_word(&words[index - 1].text)
        {
            result.push(std::mem::take(&mut current_group));
        }

        current_group.push(word.clone());
    }

    if !current_group.is_empty() {
        result.push(current_group);
    }

    result
}

fn split_long_word_units(words: &[WordUnit]) -> Vec<String> {
    let mut result = Vec::new();
    let mut queue = VecDeque::from([words.to_vec()]);

    while let Some(current_words) = queue.pop_front() {
        if current_words.is_empty() {
            continue;
        }

        let merged_text = join_word_units(&current_words);
        let max_word_count = max_segment_words_for_text(&merged_text);

        if count_words(&merged_text) <= max_word_count || current_words.len() < 4 {
            result.push(merged_text);
            continue;
        }

        let split_index = best_rule_split_index(&current_words);
        let first_words = current_words[..=split_index].to_vec();
        let second_words = current_words[split_index + 1..].to_vec();

        queue.push_back(first_words);
        queue.push_back(second_words);
    }

    result
}

fn best_rule_split_index(words: &[WordUnit]) -> usize {
    if words.len() < 2 {
        return 0;
    }

    let gaps = words
        .windows(2)
        .map(|pair| pair[1].start_time.saturating_sub(pair[0].end_time))
        .collect::<Vec<_>>();

    if gaps.iter().all(|gap| *gap == gaps[0]) {
        return words.len() / 2;
    }

    let start_index = (words.len() / 6).max(1);
    let end_index = ((5 * words.len()) / 6).min(words.len().saturating_sub(2));
    if start_index > end_index {
        return words.len() / 2;
    }

    (start_index..=end_index)
        .max_by_key(|index| gaps.get(*index).copied().unwrap_or(0))
        .unwrap_or(words.len() / 2)
}

fn ratio_threshold(max_word_count: usize, numerator: usize, denominator: usize) -> usize {
    max_word_count
        .saturating_mul(numerator)
        .checked_div(denominator.max(1))
        .unwrap_or(1)
        .max(1)
}

fn is_prefix_split_word(text: &str) -> bool {
    let normalized = text.trim().to_lowercase();
    if normalized.is_empty() {
        return false;
    }

    matches!(
        normalized.as_str(),
        "and"
            | "or"
            | "but"
            | "if"
            | "then"
            | "because"
            | "as"
            | "until"
            | "while"
            | "what"
            | "when"
            | "where"
            | "nor"
            | "yet"
            | "so"
            | "for"
            | "however"
            | "moreover"
            | "和"
            | "及"
            | "与"
            | "但"
            | "而"
            | "或"
            | "因"
            | "我"
            | "你"
            | "他"
            | "她"
            | "它"
            | "咱"
            | "您"
            | "这"
            | "那"
            | "哪"
    )
}

fn is_suffix_split_word(text: &str) -> bool {
    let normalized = text.trim().to_lowercase();
    if normalized.is_empty() {
        return false;
    }

    let suffix_words = [
        ".", ",", "!", "?", "。", "，", "！", "？", "的", "了", "着", "过", "吗", "呢", "吧", "啊",
        "呀", "嘛", "啦", "mine", "yours", "hers", "its", "ours", "theirs", "either", "neither",
    ];

    suffix_words
        .iter()
        .any(|suffix| normalized.ends_with(suffix))
}

fn max_segment_words_for_text(text: &str) -> usize {
    if is_mainly_no_space_language(text) {
        MAX_SEGMENT_WORDS_CJK
    } else {
        MAX_SEGMENT_WORDS_ENGLISH
    }
}

fn merge_word_units_by_sentences(
    words: &[WordUnit],
    sentences: &[String],
) -> Vec<TranscriptionSegment> {
    let mut word_index = 0;
    let mut segments = Vec::new();

    for sentence in sentences {
        let target_len = normalized_len(sentence);
        if target_len == 0 || word_index >= words.len() {
            continue;
        }

        let start_index = word_index;
        let mut current_len = 0;
        while word_index < words.len() && (current_len < target_len || word_index == start_index) {
            current_len += normalized_len(&words[word_index].text).max(1);
            word_index += 1;
        }

        if start_index >= word_index {
            continue;
        }

        let group = &words[start_index..word_index];
        segments.push(TranscriptionSegment {
            text: sentence.trim().to_string(),
            start_time: group.first().map(|word| word.start_time).unwrap_or(0),
            end_time: group.last().map(|word| word.end_time).unwrap_or(0),
            uid: String::new(),
            status: String::new(),
            words: group
                .iter()
                .map(|word| TranscriptionWord {
                    text: word.text.clone(),
                    start_time: word.start_time,
                    end_time: word.end_time,
                })
                .collect(),
        });
    }

    if word_index < words.len() {
        let group = &words[word_index..];
        segments.push(TranscriptionSegment {
            text: join_word_units(group),
            start_time: group.first().map(|word| word.start_time).unwrap_or(0),
            end_time: group.last().map(|word| word.end_time).unwrap_or(0),
            uid: String::new(),
            status: String::new(),
            words: group
                .iter()
                .map(|word| TranscriptionWord {
                    text: word.text.clone(),
                    start_time: word.start_time,
                    end_time: word.end_time,
                })
                .collect(),
        });
    }

    segments
}

fn parse_json_text_map(text: &str) -> Result<BTreeMap<String, String>, String> {
    let candidates = extract_json_object_candidates(text);
    if candidates.is_empty() {
        return Err("未找到 JSON 对象开始符".to_string());
    }

    let mut last_error = String::new();
    for json_text in candidates.iter().rev() {
        match parse_json_text_map_candidate(json_text) {
            Ok(result) => return Ok(result),
            Err(error) => last_error = error,
        }
    }

    Err(last_error)
}

fn parse_json_text_map_candidate(json_text: &str) -> Result<BTreeMap<String, String>, String> {
    let value = serde_json::from_str::<Value>(json_text)
        .map_err(|error| format!("JSON 解析失败: {error}"))?;
    let object = value
        .as_object()
        .ok_or_else(|| "LLM 返回内容不是 JSON 对象".to_string())?;
    let mut result = BTreeMap::new();

    for (key, value) in object {
        let text = value
            .as_str()
            .ok_or_else(|| format!("key {key} 的值不是字符串"))?;
        result.insert(key.clone(), text.to_string());
    }

    Ok(result)
}

fn extract_json_object_candidates(text: &str) -> Vec<&str> {
    let mut candidates = Vec::new();
    let mut start = None;
    let mut depth = 0usize;
    let mut in_string = false;
    let mut escaped = false;

    for (index, character) in text.char_indices() {
        if in_string {
            if escaped {
                escaped = false;
                continue;
            }

            match character {
                '\\' => escaped = true,
                '"' => in_string = false,
                _ => {}
            }
            continue;
        }

        match character {
            '"' => in_string = true,
            '{' => {
                if depth == 0 {
                    start = Some(index);
                }
                depth += 1;
            }
            '}' => {
                if depth == 0 {
                    continue;
                }

                depth -= 1;
                if depth == 0 {
                    if let Some(start_index) = start.take() {
                        candidates.push(&text[start_index..=index]);
                    }
                }
            }
            _ => {}
        }
    }

    candidates
}

fn validate_correction_keys(
    expected: &BTreeMap<String, String>,
    actual: &BTreeMap<String, String>,
) -> Result<(), String> {
    let expected_keys = expected.keys().cloned().collect::<HashSet<_>>();
    let actual_keys = actual.keys().cloned().collect::<HashSet<_>>();

    if expected_keys == actual_keys {
        return Ok(());
    }

    let missing = expected_keys
        .difference(&actual_keys)
        .cloned()
        .collect::<Vec<_>>();
    let extra = actual_keys
        .difference(&expected_keys)
        .cloned()
        .collect::<Vec<_>>();

    Err(format!("缺失 key: {:?}; 多余 key: {:?}", missing, extra))
}

fn normalize_content(text: &str) -> String {
    text.chars()
        .filter(|character| !character.is_whitespace())
        .collect()
}

fn normalized_len(text: &str) -> usize {
    normalize_content(text).chars().count()
}

fn count_words(text: &str) -> usize {
    let mut count = 0;
    let mut in_word = false;

    for character in text.chars() {
        if character.is_whitespace() {
            if in_word {
                count += 1;
                in_word = false;
            }
            continue;
        }

        if is_no_space_character(character) {
            if in_word {
                count += 1;
                in_word = false;
            }
            count += 1;
        } else {
            in_word = true;
        }
    }

    if in_word {
        count += 1;
    }

    count
}

fn is_mainly_no_space_language(text: &str) -> bool {
    let total = text
        .chars()
        .filter(|character| !character.is_whitespace())
        .count();
    if total == 0 {
        return false;
    }

    let no_space = text
        .chars()
        .filter(|character| is_no_space_character(*character))
        .count();
    (no_space as f64 / total as f64) > 0.5
}

fn is_no_space_character(character: char) -> bool {
    let code = character as u32;
    matches!(
        code,
        0x4e00..=0x9fff
            | 0x3040..=0x309f
            | 0x30a0..=0x30ff
            | 0xac00..=0xd7af
            | 0x0e00..=0x0eff
            | 0x1000..=0x109f
            | 0x1780..=0x17ff
            | 0x0900..=0x0dff
    )
}

fn estimate_max_output_tokens(text: &str) -> u32 {
    ((text.chars().count() as u32) * 6).clamp(1024, 12000)
}
