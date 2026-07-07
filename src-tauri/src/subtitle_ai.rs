use crate::ai::AiService;
use crate::app_log::LogSession;
use crate::settings::{AppSettings, SettingsStore};
use crate::transcription::{TranscriptionSegment, TranscriptionWord};
use crate::workbench_checkpoint::{
    load_checkpoint, mark_checkpoint_active, mark_checkpoint_done, mark_checkpoint_failed,
    WorkbenchCheckpointContext,
};
use futures::stream::{FuturesUnordered, StreamExt};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashSet, VecDeque};

const MAX_SEGMENT_WORDS_CJK: usize = 25;
const MAX_SEGMENT_WORDS_ENGLISH: usize = 18;
const MAX_SPLIT_CHUNK_WORDS: usize = 500;
const MAX_SPLIT_ATTEMPTS: usize = 2;
const MAX_CORRECTION_ATTEMPTS: usize = 3;
const MAX_SOURCE_REVIEW_ATTEMPTS: usize = 3;
const MAX_REFERENCE_CORRECTION_ATTEMPTS: usize = 3;
const RULE_SPLIT_GAP_MS: u64 = 500;
const RULE_MAX_GAP_MS: u64 = 1500;
const ORPHAN_FRAGMENT_GAP_MS: u64 = 650;
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
struct SourceReviewChunkResult {
    chunk: CorrectionChunk,
    entries: Vec<(usize, ReviewedSourceEntry)>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SubtitleReferenceCorrectionReference {
    pub asr_index: usize,
    pub reference_text: String,
    pub confidence: f64,
}

#[derive(Debug, Clone)]
struct ReferenceCorrectionChunk {
    start_index: usize,
    end_index: usize,
    entries: BTreeMap<String, ReferenceCorrectionPromptEntry>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ReferenceCorrectionPromptEntry {
    source_text: String,
    reference_text: String,
    confidence: f64,
}

#[derive(Debug, Clone)]
struct ReferenceCorrectionChunkResult {
    chunk: ReferenceCorrectionChunk,
    entries: Vec<(usize, ReviewedSourceEntry)>,
}

#[derive(Debug, Clone)]
struct ReviewedSourceEntry {
    text: String,
    action: String,
}

#[derive(Debug, Clone)]
struct SegmentationBlock {
    block_id: usize,
    original_segments: Vec<TranscriptionSegment>,
    display_segments: Vec<TranscriptionSegment>,
    words: Vec<WordUnit>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SegmentRangeCheckpoint {
    segments: Vec<TranscriptionSegment>,
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
    checkpoint: Option<(&SettingsStore, &WorkbenchCheckpointContext)>,
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
    let mut completed = 0usize;
    let mut checkpoint_done_blocks = HashSet::new();
    for block in &mut blocks {
        let checkpoint_key = block_checkpoint_key(block.block_id);
        if !block_needs_split_ai(block) {
            if let Some((store, context)) = checkpoint {
                let _ = mark_checkpoint_done(
                    store,
                    context,
                    &checkpoint_key,
                    &SegmentRangeCheckpoint {
                        segments: block.display_segments.clone(),
                    },
                );
            }
            completed += 1;
            continue;
        }
        if let Some((store, context)) = checkpoint {
            match load_checkpoint::<SegmentRangeCheckpoint>(store, context, &checkpoint_key) {
                Ok(Some(payload)) if !payload.segments.is_empty() => {
                    block.display_segments = payload.segments;
                    checkpoint_done_blocks.insert(block.block_id);
                    completed += 1;
                }
                Ok(_) => {}
                Err(error) => log_session.warn(
                    "smart_segmentation_checkpoint_load_failed",
                    "读取智能断句检查点失败，将重新执行批次",
                    json!({ "blockIndex": block.block_id + 1, "error": error }),
                ),
            }
        }
    }
    while next_block_index < blocks.len() && split_futures.len() < max_active {
        if !block_needs_split_ai(&blocks[next_block_index])
            || checkpoint_done_blocks.contains(&blocks[next_block_index].block_id)
        {
            next_block_index += 1;
            continue;
        }

        let block = &mut blocks[next_block_index];
        set_segments_status(&mut block.display_segments, "segmenting");

        let block_id = block.block_id;
        if let Some((store, context)) = checkpoint {
            let _ = mark_checkpoint_active(store, context, &block_checkpoint_key(block_id));
        }
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
                    if let Some((store, context)) = checkpoint {
                        let _ = mark_checkpoint_done(
                            store,
                            context,
                            &block_checkpoint_key(block_id),
                            &SegmentRangeCheckpoint {
                                segments: block.display_segments.clone(),
                            },
                        );
                    }
                }
                Ok(_) => {
                    block.display_segments = block.original_segments.clone();
                    set_segments_status(&mut block.display_segments, "kept");
                    failed_blocks += 1;
                    if let Some((store, context)) = checkpoint {
                        let _ = mark_checkpoint_failed(
                            store,
                            context,
                            &block_checkpoint_key(block_id),
                            "智能断句批次未返回可用结果",
                        );
                    }
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
                    if let Some((store, context)) = checkpoint {
                        let _ = mark_checkpoint_failed(
                            store,
                            context,
                            &block_checkpoint_key(block_id),
                            &error,
                        );
                    }
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
            if !block_needs_split_ai(&blocks[next_block_index])
                || checkpoint_done_blocks.contains(&blocks[next_block_index].block_id)
            {
                next_block_index += 1;
                continue;
            }

            let block = &mut blocks[next_block_index];
            set_segments_status(&mut block.display_segments, "segmenting");

            let block_id = block.block_id;
            if let Some((store, context)) = checkpoint {
                let _ = mark_checkpoint_active(store, context, &block_checkpoint_key(block_id));
            }
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
    checkpoint: Option<(&SettingsStore, &WorkbenchCheckpointContext)>,
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
    let mut completed = 0usize;
    let mut checkpoint_done_chunks = HashSet::new();
    if let Some((store, context)) = checkpoint {
        for chunk in &chunks {
            let checkpoint_key = chunk_checkpoint_key(chunk.start_index, chunk.end_index);
            match load_checkpoint::<SegmentRangeCheckpoint>(store, context, &checkpoint_key) {
                Ok(Some(payload)) if !payload.segments.is_empty() => {
                    apply_checkpoint_segments(
                        &mut corrected_segments,
                        chunk.start_index,
                        payload.segments,
                    );
                    checkpoint_done_chunks.insert(chunk.start_index);
                    completed += 1;
                }
                Ok(_) => {}
                Err(error) => log_session.warn(
                    "subtitle_correction_checkpoint_load_failed",
                    "读取字幕校正检查点失败，将重新执行批次",
                    json!({
                        "startIndex": chunk.start_index + 1,
                        "endIndex": chunk.end_index + 1,
                        "error": error,
                    }),
                ),
            }
        }
    }
    while next_chunk_index < chunks.len() && correction_futures.len() < max_active {
        let chunk = chunks[next_chunk_index].clone();
        if checkpoint_done_chunks.contains(&chunk.start_index) {
            next_chunk_index += 1;
            continue;
        }
        mark_range_status(
            &mut corrected_segments,
            chunk.start_index,
            chunk.end_index,
            "correcting",
        );
        if let Some((store, context)) = checkpoint {
            let _ = mark_checkpoint_active(
                store,
                context,
                &chunk_checkpoint_key(chunk.start_index, chunk.end_index),
            );
        }
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
    report(
        stage_progress(0, 100, completed, total),
        "AI 字幕校正中",
        &corrected_segments,
        &warnings,
    );

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
                if let Some((store, context)) = checkpoint {
                    let _ = mark_checkpoint_done(
                        store,
                        context,
                        &chunk_checkpoint_key(result.chunk.start_index, result.chunk.end_index),
                        &segment_range_checkpoint(
                            &corrected_segments,
                            result.chunk.start_index,
                            result.chunk.end_index,
                        ),
                    );
                }
            }
            Err((chunk, error)) => {
                mark_range_status(
                    &mut corrected_segments,
                    chunk.start_index,
                    chunk.end_index,
                    "kept",
                );
                failed_chunks += 1;
                if let Some((store, context)) = checkpoint {
                    let _ = mark_checkpoint_failed(
                        store,
                        context,
                        &chunk_checkpoint_key(chunk.start_index, chunk.end_index),
                        &error,
                    );
                }
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
            if checkpoint_done_chunks.contains(&chunk.start_index) {
                next_chunk_index += 1;
                continue;
            }
            mark_range_status(
                &mut corrected_segments,
                chunk.start_index,
                chunk.end_index,
                "correcting",
            );
            if let Some((store, context)) = checkpoint {
                let _ = mark_checkpoint_active(
                    store,
                    context,
                    &chunk_checkpoint_key(chunk.start_index, chunk.end_index),
                );
            }
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

pub async fn review_source_subtitles<F>(
    settings: &AppSettings,
    ai_service: &AiService,
    log_session: &LogSession,
    segments: Vec<TranscriptionSegment>,
    report: &mut F,
    checkpoint: Option<(&SettingsStore, &WorkbenchCheckpointContext)>,
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

    let chunks =
        build_correction_chunks(&segments, settings.translation_batch_size.max(1) as usize);
    let mut reviewed_segments = segments;
    let mut failed_chunks = 0usize;
    let mut warnings = Vec::new();

    if chunks.is_empty() {
        return SubtitleProcessingResult {
            segments: reviewed_segments,
            warnings,
        };
    }

    log_session.info(
        "source_subtitle_review_stage_prepared",
        "AI 源文审核批次已准备",
        json!({
            "inputSegmentCount": reviewed_segments.len(),
            "chunkCount": chunks.len(),
            "batchSize": settings.translation_batch_size.max(1),
            "videoContentType": &settings.video_content_type,
            "reviewMode": &settings.ai_subtitle_review_mode,
            "llmMode": "configured_llm_settings_json_response",
        }),
    );

    let total = chunks.len().max(1);
    let max_active = active_ai_work_count(settings);
    let mut futures = FuturesUnordered::new();
    let mut next_chunk_index = 0usize;
    let mut completed = 0usize;
    let mut checkpoint_done_chunks = HashSet::new();

    if let Some((store, context)) = checkpoint {
        for chunk in &chunks {
            let checkpoint_key = chunk_checkpoint_key(chunk.start_index, chunk.end_index);
            match load_checkpoint::<SegmentRangeCheckpoint>(store, context, &checkpoint_key) {
                Ok(Some(payload)) if !payload.segments.is_empty() => {
                    apply_checkpoint_segments(
                        &mut reviewed_segments,
                        chunk.start_index,
                        payload.segments,
                    );
                    checkpoint_done_chunks.insert(chunk.start_index);
                    completed += 1;
                }
                Ok(_) => {}
                Err(error) => log_session.warn(
                    "source_subtitle_review_checkpoint_load_failed",
                    "读取源文审核检查点失败，将重新执行批次",
                    json!({
                        "startIndex": chunk.start_index + 1,
                        "endIndex": chunk.end_index + 1,
                        "error": error,
                    }),
                ),
            }
        }
    }

    while next_chunk_index < chunks.len() && futures.len() < max_active {
        let chunk = chunks[next_chunk_index].clone();
        if checkpoint_done_chunks.contains(&chunk.start_index) {
            next_chunk_index += 1;
            continue;
        }
        mark_range_status(
            &mut reviewed_segments,
            chunk.start_index,
            chunk.end_index,
            "reviewing",
        );
        if let Some((store, context)) = checkpoint {
            let _ = mark_checkpoint_active(
                store,
                context,
                &chunk_checkpoint_key(chunk.start_index, chunk.end_index),
            );
        }
        futures.push(run_source_review_chunk(
            settings,
            ai_service,
            chunk,
            log_session.clone(),
        ));
        next_chunk_index += 1;
    }
    report(
        stage_progress(0, 100, completed, total),
        "AI 源文审核中",
        &reviewed_segments,
        &warnings,
    );

    while let Some(result) = futures.next().await {
        completed += 1;

        match result {
            Ok(result) => {
                for (index, entry) in result.entries {
                    if let Some(segment) = reviewed_segments.get_mut(index) {
                        segment.text = entry.text;
                        segment.status = if entry.action == "remove" {
                            "removed".to_string()
                        } else {
                            "reviewed".to_string()
                        };
                    }
                }
                mark_unfinished_review_range(
                    &mut reviewed_segments,
                    result.chunk.start_index,
                    result.chunk.end_index,
                    "reviewed",
                );
                if let Some((store, context)) = checkpoint {
                    let _ = mark_checkpoint_done(
                        store,
                        context,
                        &chunk_checkpoint_key(result.chunk.start_index, result.chunk.end_index),
                        &segment_range_checkpoint(
                            &reviewed_segments,
                            result.chunk.start_index,
                            result.chunk.end_index,
                        ),
                    );
                }
            }
            Err((chunk, error)) => {
                mark_range_status(
                    &mut reviewed_segments,
                    chunk.start_index,
                    chunk.end_index,
                    "kept",
                );
                failed_chunks += 1;
                if let Some((store, context)) = checkpoint {
                    let _ = mark_checkpoint_failed(
                        store,
                        context,
                        &chunk_checkpoint_key(chunk.start_index, chunk.end_index),
                        &error,
                    );
                }
                log_session.warn(
                    "source_subtitle_review_chunk_failed",
                    "源文审核批次失败，已保留原文",
                    json!({
                        "startIndex": chunk.start_index + 1,
                        "endIndex": chunk.end_index + 1,
                        "entryCount": chunk.entries.len(),
                        "error": &error,
                    }),
                );
            }
        }

        while next_chunk_index < chunks.len() && futures.len() < max_active {
            let chunk = chunks[next_chunk_index].clone();
            if checkpoint_done_chunks.contains(&chunk.start_index) {
                next_chunk_index += 1;
                continue;
            }
            mark_range_status(
                &mut reviewed_segments,
                chunk.start_index,
                chunk.end_index,
                "reviewing",
            );
            if let Some((store, context)) = checkpoint {
                let _ = mark_checkpoint_active(
                    store,
                    context,
                    &chunk_checkpoint_key(chunk.start_index, chunk.end_index),
                );
            }
            futures.push(run_source_review_chunk(
                settings,
                ai_service,
                chunk,
                log_session.clone(),
            ));
            next_chunk_index += 1;
        }

        warnings = build_processing_warnings("AI源文审核", failed_chunks, "审核批次");
        let progress = stage_progress(0, 100, completed, total);
        let message = if completed == total {
            "AI 源文审核完成"
        } else {
            "AI 源文审核中"
        };
        report(progress, message, &reviewed_segments, &warnings);
    }

    if failed_chunks > 0 {
        log_session.warn(
            "source_subtitle_review_stage_partial",
            "AI 源文审核部分批次失败，已保留原文",
            json!({
                "failedChunkCount": failed_chunks,
                "chunkCount": total,
            }),
        );
    }

    SubtitleProcessingResult {
        segments: reviewed_segments,
        warnings,
    }
}

pub async fn correct_subtitles_with_downloaded_reference<F>(
    settings: &AppSettings,
    ai_service: &AiService,
    log_session: &LogSession,
    segments: Vec<TranscriptionSegment>,
    references: &[SubtitleReferenceCorrectionReference],
    report: &mut F,
    checkpoint: Option<(&SettingsStore, &WorkbenchCheckpointContext)>,
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

    let chunks = build_reference_correction_chunks(
        &segments,
        references,
        settings.translation_batch_size.max(1) as usize,
    );
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
        "subtitle_reference_correction_stage_prepared",
        "AI 参考校正批次已准备",
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
    let mut futures = FuturesUnordered::new();
    let mut next_chunk_index = 0usize;
    let mut completed = 0usize;
    let mut checkpoint_done_chunks = HashSet::new();

    if let Some((store, context)) = checkpoint {
        for chunk in &chunks {
            let checkpoint_key = chunk_checkpoint_key(chunk.start_index, chunk.end_index);
            match load_checkpoint::<SegmentRangeCheckpoint>(store, context, &checkpoint_key) {
                Ok(Some(payload)) if !payload.segments.is_empty() => {
                    apply_checkpoint_segments(
                        &mut corrected_segments,
                        chunk.start_index,
                        payload.segments,
                    );
                    checkpoint_done_chunks.insert(chunk.start_index);
                    completed += 1;
                }
                Ok(_) => {}
                Err(error) => log_session.warn(
                    "subtitle_reference_correction_checkpoint_load_failed",
                    "读取参考校正检查点失败，将重新执行批次",
                    json!({
                        "startIndex": chunk.start_index + 1,
                        "endIndex": chunk.end_index + 1,
                        "error": error,
                    }),
                ),
            }
        }
    }

    while next_chunk_index < chunks.len() && futures.len() < max_active {
        let chunk = chunks[next_chunk_index].clone();
        if checkpoint_done_chunks.contains(&chunk.start_index) {
            next_chunk_index += 1;
            continue;
        }
        mark_range_status(
            &mut corrected_segments,
            chunk.start_index,
            chunk.end_index,
            "correcting",
        );
        if let Some((store, context)) = checkpoint {
            let _ = mark_checkpoint_active(
                store,
                context,
                &chunk_checkpoint_key(chunk.start_index, chunk.end_index),
            );
        }
        futures.push(run_reference_correction_chunk(
            settings,
            ai_service,
            chunk,
            log_session.clone(),
        ));
        next_chunk_index += 1;
    }
    report(
        stage_progress(0, 100, completed, total),
        "AI 参考校正中",
        &corrected_segments,
        &warnings,
    );

    while let Some(result) = futures.next().await {
        completed += 1;

        match result {
            Ok(result) => {
                for (index, entry) in result.entries {
                    if let Some(segment) = corrected_segments.get_mut(index) {
                        segment.text = entry.text;
                        segment.status = match entry.action.as_str() {
                            "remove" => "removed",
                            "keep" => "kept",
                            _ => "corrected",
                        }
                        .to_string();
                    }
                }
                if let Some((store, context)) = checkpoint {
                    let _ = mark_checkpoint_done(
                        store,
                        context,
                        &chunk_checkpoint_key(result.chunk.start_index, result.chunk.end_index),
                        &segment_range_checkpoint(
                            &corrected_segments,
                            result.chunk.start_index,
                            result.chunk.end_index,
                        ),
                    );
                }
            }
            Err((chunk, error)) => {
                mark_range_status(
                    &mut corrected_segments,
                    chunk.start_index,
                    chunk.end_index,
                    "kept",
                );
                failed_chunks += 1;
                if let Some((store, context)) = checkpoint {
                    let _ = mark_checkpoint_failed(
                        store,
                        context,
                        &chunk_checkpoint_key(chunk.start_index, chunk.end_index),
                        &error,
                    );
                }
                log_session.warn(
                    "subtitle_reference_correction_chunk_failed",
                    "参考校正批次失败，已保留转录字幕",
                    json!({
                        "startIndex": chunk.start_index + 1,
                        "endIndex": chunk.end_index + 1,
                        "entryCount": chunk.entries.len(),
                        "error": &error,
                    }),
                );
            }
        }

        while next_chunk_index < chunks.len() && futures.len() < max_active {
            let chunk = chunks[next_chunk_index].clone();
            if checkpoint_done_chunks.contains(&chunk.start_index) {
                next_chunk_index += 1;
                continue;
            }
            mark_range_status(
                &mut corrected_segments,
                chunk.start_index,
                chunk.end_index,
                "correcting",
            );
            if let Some((store, context)) = checkpoint {
                let _ = mark_checkpoint_active(
                    store,
                    context,
                    &chunk_checkpoint_key(chunk.start_index, chunk.end_index),
                );
            }
            futures.push(run_reference_correction_chunk(
                settings,
                ai_service,
                chunk,
                log_session.clone(),
            ));
            next_chunk_index += 1;
        }

        warnings = build_processing_warnings("参考校正", failed_chunks, "校正批次");
        let progress = stage_progress(0, 100, completed, total);
        let message = if completed == total {
            "AI 参考校正完成"
        } else {
            "AI 参考校正中"
        };
        report(progress, message, &corrected_segments, &warnings);
    }

    if failed_chunks > 0 {
        log_session.warn(
            "subtitle_reference_correction_stage_partial",
            "AI 参考校正部分批次失败，已保留转录字幕",
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

fn block_checkpoint_key(block_id: usize) -> String {
    format!("block-{block_id}")
}

fn chunk_checkpoint_key(start_index: usize, end_index: usize) -> String {
    format!("chunk-{start_index}-{end_index}")
}

fn segment_range_checkpoint(
    segments: &[TranscriptionSegment],
    start_index: usize,
    end_index: usize,
) -> SegmentRangeCheckpoint {
    if start_index >= segments.len() {
        return SegmentRangeCheckpoint {
            segments: Vec::new(),
        };
    }
    let end_index = end_index.min(segments.len().saturating_sub(1));
    SegmentRangeCheckpoint {
        segments: segments[start_index..=end_index].to_vec(),
    }
}

fn apply_checkpoint_segments(
    segments: &mut [TranscriptionSegment],
    start_index: usize,
    checkpoint_segments: Vec<TranscriptionSegment>,
) {
    for (offset, checkpoint_segment) in checkpoint_segments.into_iter().enumerate() {
        if let Some(segment) = segments.get_mut(start_index + offset) {
            *segment = checkpoint_segment;
        }
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

async fn run_source_review_chunk(
    settings: &AppSettings,
    ai_service: &AiService,
    chunk: CorrectionChunk,
    log_session: LogSession,
) -> Result<SourceReviewChunkResult, (CorrectionChunk, String)> {
    review_source_chunk_by_llm(settings, ai_service, chunk, log_session).await
}

async fn run_reference_correction_chunk(
    settings: &AppSettings,
    ai_service: &AiService,
    chunk: ReferenceCorrectionChunk,
    log_session: LogSession,
) -> Result<ReferenceCorrectionChunkResult, (ReferenceCorrectionChunk, String)> {
    correct_reference_chunk_by_llm(settings, ai_service, chunk, log_session).await
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

async fn review_source_chunk_by_llm(
    settings: &AppSettings,
    ai_service: &AiService,
    chunk: CorrectionChunk,
    log_session: LogSession,
) -> Result<SourceReviewChunkResult, (CorrectionChunk, String)> {
    let system_prompt = build_source_review_system_prompt(settings);
    let max_output_tokens = estimate_max_output_tokens(
        &chunk
            .entries
            .values()
            .cloned()
            .collect::<Vec<_>>()
            .join("\n"),
    );
    let mut feedback = String::new();

    for attempt in 1..=MAX_SOURCE_REVIEW_ATTEMPTS {
        let user_prompt = build_source_review_user_prompt(&chunk.entries, &feedback);
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
                    "source_subtitle_review_llm_request_failed",
                    "源文审核 LLM 请求失败",
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

        let parsed = match parse_source_review_response(&response) {
            Ok(parsed) => parsed,
            Err(error) => {
                feedback = build_source_review_json_feedback(&chunk.entries, &error);
                log_session.warn(
                    "source_subtitle_review_validation_failed",
                    "源文审核 LLM 结果校验失败，准备带反馈重试",
                    json!({
                        "attempt": attempt,
                        "startIndex": chunk.start_index + 1,
                        "endIndex": chunk.end_index + 1,
                        "validationType": "json_parse",
                        "error": &error,
                    }),
                );
                continue;
            }
        };

        match validate_source_review_keys(&chunk.entries, &parsed) {
            Ok(()) => {
                let entries = parsed
                    .into_iter()
                    .filter_map(|(key, entry)| {
                        key.parse::<usize>().ok().map(|index| (index - 1, entry))
                    })
                    .collect();
                return Ok(SourceReviewChunkResult { chunk, entries });
            }
            Err(error) => {
                feedback = build_source_review_key_feedback(&chunk.entries, &error);
                log_session.warn(
                    "source_subtitle_review_validation_failed",
                    "源文审核 LLM 结果校验失败，准备带反馈重试",
                    json!({
                        "attempt": attempt,
                        "startIndex": chunk.start_index + 1,
                        "endIndex": chunk.end_index + 1,
                        "validationType": "key_mismatch",
                        "error": &error,
                    }),
                );
            }
        }
    }

    Err((chunk, "LLM 源文审核结果多次校验失败".to_string()))
}

async fn correct_reference_chunk_by_llm(
    settings: &AppSettings,
    ai_service: &AiService,
    chunk: ReferenceCorrectionChunk,
    log_session: LogSession,
) -> Result<ReferenceCorrectionChunkResult, (ReferenceCorrectionChunk, String)> {
    let system_prompt = build_reference_correction_system_prompt(settings);
    let max_output_tokens = estimate_max_output_tokens(
        &chunk
            .entries
            .values()
            .map(|entry| {
                format!(
                    "{}\n{}",
                    entry.source_text.trim(),
                    entry.reference_text.trim()
                )
            })
            .collect::<Vec<_>>()
            .join("\n"),
    );
    let mut feedback = String::new();

    for attempt in 1..=MAX_REFERENCE_CORRECTION_ATTEMPTS {
        let user_prompt = build_reference_correction_user_prompt(&chunk.entries, &feedback);
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
                    "subtitle_reference_correction_llm_request_failed",
                    "参考校正 LLM 请求失败",
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

        let parsed = match parse_source_review_response(&response) {
            Ok(parsed) => parsed,
            Err(error) => {
                feedback = build_reference_correction_json_feedback(&chunk.entries, &error);
                log_session.warn(
                    "subtitle_reference_correction_validation_failed",
                    "参考校正 LLM 结果校验失败，准备带反馈重试",
                    json!({
                        "attempt": attempt,
                        "startIndex": chunk.start_index + 1,
                        "endIndex": chunk.end_index + 1,
                        "validationType": "json_parse",
                        "error": &error,
                    }),
                );
                continue;
            }
        };

        match validate_reference_correction_keys(&chunk.entries, &parsed) {
            Ok(()) => {
                let entries = parsed
                    .into_iter()
                    .filter_map(|(key, entry)| {
                        key.parse::<usize>().ok().map(|index| (index - 1, entry))
                    })
                    .collect();
                return Ok(ReferenceCorrectionChunkResult { chunk, entries });
            }
            Err(error) => {
                feedback = build_reference_correction_key_feedback(&chunk.entries, &error);
                log_session.warn(
                    "subtitle_reference_correction_validation_failed",
                    "参考校正 LLM 结果校验失败，准备带反馈重试",
                    json!({
                        "attempt": attempt,
                        "startIndex": chunk.start_index + 1,
                        "endIndex": chunk.end_index + 1,
                        "validationType": "key_mismatch",
                        "error": &error,
                    }),
                );
            }
        }
    }

    Err((chunk, "LLM 参考校正结果多次校验失败".to_string()))
}

fn build_source_review_system_prompt(settings: &AppSettings) -> String {
    let mode_rule = if settings.ai_subtitle_review_mode == "conservative" {
        "保守模式：严格保持每条字幕的独立含义，不删除内容；只有明显 ASR 错字、术语误识别、口癖、标点和非语言声音可修正。"
    } else {
        "专家模式：以最终字幕质量为第一目标。可以删除无意义噪音行、支付提示、系统音、背景杂音、重复废话；可以修正上下文高度明确的 ASR 误识别和术语错误。"
    };
    let reference = match settings.video_content_type.as_str() {
        "trading" => "交易视频：重点保护数字、价格、百分比、ticker、交易方向、周期、指标和 Al Brooks 价格行为术语。macro channel/gap、macro E-mini 在交易语境中应修正为 micro channel/gap、micro E-mini。",
        _ => "通用视频：优先保证语义准确、上下文通顺、字幕可读。术语按上下文最小改动修正。",
    };

    format!(
        r#"你是一位专业 AI 字幕审核专家，正在审核 ASR 转录后的源文字幕。

<review_mode>{mode_rule}</review_mode>
<domain_reference>{reference}</domain_reference>

<rules>
1. 保持输入 JSON 的所有真实 key，不新增、不删除、不重命名 key。
2. 每个 key 输出对象，必须包含 text 和 action 两个字段。
3. action 只能是 keep、revise、remove。
4. keep 表示原文可接受，text 原样保留；revise 表示修正源文，text 写修正后的原文；remove 表示该行是噪音或无意义内容，text 可保留原文或写空字符串。
5. 只修正上下文高度明确的问题，不要凭空补充音频中不存在的信息。
6. 不翻译源文，不改变数字、专有名词、方向性判断、风险提示和事实。
7. 支付成功、到账、系统通知、背景提示音、无意义拟声词、掌声笑声等与主体内容无关的转录行，在专家模式下标记 remove。
8. 输出只能是单个 JSON object，第一字符必须是 {{，最后字符必须是 }}；禁止 Markdown、解释、代码块或额外文本。
</rules>

<output_format>
{{
  "1": {{ "text": "审核后的源文", "action": "keep" }}
}}
</output_format>"#
    )
}

fn build_reference_correction_system_prompt(settings: &AppSettings) -> String {
    let mode_rule = if settings.ai_subtitle_review_mode == "conservative" {
        "保守模式：下载字幕只作为弱参考。只有转录明显错听、漏词、术语误识别、标点或噪音时才修改；不确定时 keep。"
    } else {
        "专家模式：以转录字幕的时间轴和口播内容为主，结合下载字幕参考修正明显 ASR 错误、漏识别、术语、人名和标点；可删除明显噪音行。"
    };
    let reference = match settings.video_content_type.as_str() {
        "trading" => "交易视频：重点保护数字、价格、百分比、ticker、交易方向、周期、指标和 Al Brooks 价格行为术语。参考字幕可用于修正交易术语，但不得改变交易判断。",
        _ => "通用视频：优先保证语义准确、上下文通顺、字幕可读。下载字幕只用于辅助判断，不是最终答案来源。",
    };

    format!(
        r#"你是一位专业字幕参考校正专家。你正在处理 ASR 转录字幕，每条转录字幕已经有正确时间轴；下载字幕只是参考资料。

<review_mode>{mode_rule}</review_mode>
<domain_reference>{reference}</domain_reference>

<rules>
1. 转录字幕是主依据，下载字幕只是参考；冲突、不确定、语言不一致或参考缺失时必须保留转录。
2. 保持输入 JSON 的所有真实 key，不新增、不删除、不重命名 key，不合并、不拆分条目。
3. 只能修改每个 key 的 text 和 action，不能输出时间戳、解释、Markdown 或额外字段。
4. action 只能是 keep、revise、remove。
5. keep 表示转录可接受，text 原样保留；revise 表示参考字幕能证明转录有明确错误，text 写校正后的源文；remove 表示该转录行是噪音或无意义内容。
6. 不翻译源文，不扩写下载字幕中有但音频/转录上下文不支持的内容。
7. 不改变数字、专有名词、方向性判断、风险提示和事实；只有参考和上下文都明确时才修正。
8. 输出只能是单个 JSON object，第一字符必须是 {{，最后字符必须是 }}。
</rules>

<output_format>
{{
  "1": {{ "text": "校正后的源文", "action": "keep" }}
}}
</output_format>"#
    )
}

fn build_source_review_user_prompt(entries: &BTreeMap<String, String>, feedback: &str) -> String {
    let input_json = serde_json::to_string(entries).unwrap_or_else(|_| "{}".to_string());
    let output_template = entries
        .iter()
        .map(|(key, text)| {
            (
                key.clone(),
                json!({
                    "text": text,
                    "action": "keep",
                }),
            )
        })
        .collect::<serde_json::Map<_, _>>();
    let output_template = Value::Object(output_template).to_string();
    let mut prompt = format!(
        "请审核以下源文字幕 JSON。最终必须输出 JSON object，key 必须与输入完全一致。\n\
         <source_subtitle>{input_json}</source_subtitle>\n\
         <output_template>{output_template}</output_template>\n\
         <template_rule>最终答案必须复制 output_template 的完整 JSON object 外层结构和全部 key，只改每个 key 下的 text 和 action。</template_rule>\n\
         <final_answer_rule>最终答案第一字符必须是 {{，最后字符必须是 }}，且必须能被 JSON.parse 直接解析。</final_answer_rule>"
    );

    if !feedback.is_empty() {
        prompt.push_str("\n<feedback>");
        prompt.push_str(feedback);
        prompt.push_str("</feedback>");
    }

    prompt
}

fn build_reference_correction_user_prompt(
    entries: &BTreeMap<String, ReferenceCorrectionPromptEntry>,
    feedback: &str,
) -> String {
    let input_json = serde_json::to_string(entries).unwrap_or_else(|_| "{}".to_string());
    let output_template = entries
        .iter()
        .map(|(key, entry)| {
            (
                key.clone(),
                json!({
                    "text": entry.source_text,
                    "action": "keep",
                }),
            )
        })
        .collect::<serde_json::Map<_, _>>();
    let output_template = Value::Object(output_template).to_string();
    let mut prompt = format!(
        "请根据下载字幕参考校正以下 ASR 转录字幕 JSON。最终必须输出 JSON object，key 必须与输入完全一致。\n\
         每个输入项包含 sourceText（转录主文本）、referenceText（下载字幕参考）和 confidence（参考匹配置信度）。\n\
         <subtitle_items>{input_json}</subtitle_items>\n\
         <output_template>{output_template}</output_template>\n\
         <template_rule>最终答案必须复制 output_template 的完整 JSON object 外层结构和全部 key，只改每个 key 下的 text 和 action。</template_rule>\n\
         <final_answer_rule>最终答案第一字符必须是 {{，最后字符必须是 }}，且必须能被 JSON.parse 直接解析。</final_answer_rule>"
    );

    if !feedback.is_empty() {
        prompt.push_str("\n<feedback>");
        prompt.push_str(feedback);
        prompt.push_str("</feedback>");
    }

    prompt
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

fn build_source_review_json_feedback(entries: &BTreeMap<String, String>, error: &str) -> String {
    let output_template = source_review_output_template(entries);

    format!(
        "上一次结果不是有效源文审核 JSON: {error}\n请只输出完整 JSON 对象，第一字符必须是 {{，最后字符必须是 }}。每个 key 的 value 必须是包含 text 和 action 的对象，action 只能是 keep、revise、remove。请复制这个结构: {output_template}"
    )
}

fn build_source_review_key_feedback(entries: &BTreeMap<String, String>, error: &str) -> String {
    let output_template = source_review_output_template(entries);

    format!(
        "上一次结果 key 不匹配: {error}\n请输出完整 JSON，必须包含原始所有 key，不能新增、遗漏或重命名。请复制这个结构: {output_template}"
    )
}

fn build_reference_correction_json_feedback(
    entries: &BTreeMap<String, ReferenceCorrectionPromptEntry>,
    error: &str,
) -> String {
    let output_template = reference_correction_output_template(entries);

    format!(
        "上一次结果不是有效参考校正 JSON: {error}\n请只输出完整 JSON 对象，第一字符必须是 {{，最后字符必须是 }}。每个 key 的 value 必须是包含 text 和 action 的对象，action 只能是 keep、revise、remove。请复制这个结构: {output_template}"
    )
}

fn build_reference_correction_key_feedback(
    entries: &BTreeMap<String, ReferenceCorrectionPromptEntry>,
    error: &str,
) -> String {
    let output_template = reference_correction_output_template(entries);

    format!(
        "上一次结果 key 不匹配: {error}\n请输出完整 JSON，必须包含原始所有 key，不能新增、遗漏或重命名。请复制这个结构: {output_template}"
    )
}

fn source_review_output_template(entries: &BTreeMap<String, String>) -> String {
    let template = entries
        .iter()
        .map(|(key, text)| {
            (
                key.clone(),
                json!({
                    "text": text,
                    "action": "keep",
                }),
            )
        })
        .collect::<serde_json::Map<_, _>>();

    Value::Object(template).to_string()
}

fn reference_correction_output_template(
    entries: &BTreeMap<String, ReferenceCorrectionPromptEntry>,
) -> String {
    let template = entries
        .iter()
        .map(|(key, entry)| {
            (
                key.clone(),
                json!({
                    "text": entry.source_text,
                    "action": "keep",
                }),
            )
        })
        .collect::<serde_json::Map<_, _>>();

    Value::Object(template).to_string()
}

fn build_word_units(segments: &[TranscriptionSegment]) -> Vec<WordUnit> {
    segments
        .iter()
        .flat_map(|segment| {
            if segment.words.is_empty() {
                estimate_word_units(segment)
            } else {
                let mut words = segment
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
                    apply_segment_timing_bounds(&mut words, segment);
                    words
                }
            }
        })
        .collect()
}

fn apply_segment_timing_bounds(words: &mut [WordUnit], segment: &TranscriptionSegment) {
    if words.is_empty() {
        return;
    }

    if let Some(first) = words.first_mut() {
        first.start_time = first.start_time.max(segment.start_time);
        if first.end_time < first.start_time {
            first.end_time = first.start_time;
        }
    }

    if let Some(last) = words.last_mut() {
        last.end_time = last.end_time.max(segment.end_time);
        if last.start_time > last.end_time {
            last.start_time = last.end_time;
        }
    }
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

fn build_reference_correction_chunks(
    segments: &[TranscriptionSegment],
    references: &[SubtitleReferenceCorrectionReference],
    batch_size: usize,
) -> Vec<ReferenceCorrectionChunk> {
    let mut reference_by_index = BTreeMap::new();
    for reference in references {
        reference_by_index.insert(reference.asr_index, reference);
    }

    segments
        .chunks(batch_size.max(1))
        .enumerate()
        .map(|(chunk_index, chunk)| {
            let start_index = chunk_index * batch_size.max(1);
            let mut entries = BTreeMap::new();

            for (offset, segment) in chunk.iter().enumerate() {
                let index = start_index + offset;
                let reference = reference_by_index.get(&index);
                entries.insert(
                    (index + 1).to_string(),
                    ReferenceCorrectionPromptEntry {
                        source_text: segment.text.clone(),
                        reference_text: reference
                            .map(|item| item.reference_text.clone())
                            .unwrap_or_default(),
                        confidence: reference.map(|item| item.confidence).unwrap_or_default(),
                    },
                );
            }

            ReferenceCorrectionChunk {
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

    repair_split_sentences_by_word_rules(&normalized, words)
}

fn repair_split_sentences_by_word_rules(
    sentences: &[String],
    words: &[WordUnit],
) -> Result<Vec<String>, String> {
    let mut groups = word_groups_by_sentences(words, sentences)?;
    let mut index = 0usize;
    while index + 1 < groups.len() {
        move_prefix_words_to_complete_previous(&mut groups, index);
        index += 1;
    }

    let mut repaired: Vec<Vec<WordUnit>> = Vec::new();
    for group in groups.into_iter().filter(|group| !group.is_empty()) {
        let should_merge = repaired
            .last()
            .map(|previous| should_merge_short_fragment(previous, &group))
            .unwrap_or(false);
        if should_merge {
            if let Some(previous) = repaired.last_mut() {
                previous.extend(group);
            }
        } else {
            repaired.push(group);
        }
    }

    Ok(repaired
        .iter()
        .map(|group| join_word_units(group))
        .filter(|text| !text.trim().is_empty())
        .collect())
}

fn word_groups_by_sentences(
    words: &[WordUnit],
    sentences: &[String],
) -> Result<Vec<Vec<WordUnit>>, String> {
    if words.is_empty() {
        return Ok(Vec::new());
    }

    let mut groups = Vec::new();
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

        if start_index < word_index {
            groups.push(words[start_index..word_index].to_vec());
        }
    }

    if word_index < words.len() {
        groups.push(words[word_index..].to_vec());
    }

    if groups.is_empty() {
        Err("断句结果无法映射到词级时间轴".to_string())
    } else {
        Ok(groups)
    }
}

fn move_prefix_words_to_complete_previous(groups: &mut [Vec<WordUnit>], index: usize) {
    if index + 1 >= groups.len() || groups[index].is_empty() || groups[index + 1].is_empty() {
        return;
    }

    let mut moved = 0usize;
    while moved < 4 && !groups[index + 1].is_empty() {
        let previous_text = join_word_units(&groups[index]);
        if !needs_previous_completion(&previous_text) {
            break;
        }

        let next_word = groups[index + 1][0].clone();
        if !can_move_prefix_word(&groups[index], &groups[index + 1], &next_word) {
            break;
        }

        let moved_word = groups[index + 1].remove(0);
        groups[index].push(moved_word);
        moved += 1;

        if is_terminal_text(&next_word.text) {
            break;
        }
    }
}

fn should_merge_short_fragment(previous: &[WordUnit], current: &[WordUnit]) -> bool {
    if previous.is_empty() || current.is_empty() {
        return false;
    }

    let previous_text = join_word_units(previous);
    let current_text = join_word_units(current);
    if !is_short_orphan_fragment(&current_text) && !needs_previous_completion(&previous_text) {
        return false;
    }

    let gap = current
        .first()
        .map(|first| first.start_time)
        .unwrap_or_default()
        .saturating_sub(
            previous
                .last()
                .map(|last| last.end_time)
                .unwrap_or_default(),
        );
    if gap > ORPHAN_FRAGMENT_GAP_MS && is_terminal_text(&previous_text) {
        return false;
    }

    let merged_words = previous
        .iter()
        .chain(current.iter())
        .cloned()
        .collect::<Vec<_>>();
    let merged_text = join_word_units(&merged_words);
    count_words(&merged_text) <= max_segment_words_for_text(&merged_text)
}

fn can_move_prefix_word(
    previous: &[WordUnit],
    next_group: &[WordUnit],
    next_word: &WordUnit,
) -> bool {
    let gap = next_word.start_time.saturating_sub(
        previous
            .last()
            .map(|word| word.end_time)
            .unwrap_or_default(),
    );
    if gap > ORPHAN_FRAGMENT_GAP_MS {
        return false;
    }

    let next_text = next_word.text.trim();
    let previous_text = join_word_units(previous);
    let candidate_words = previous
        .iter()
        .cloned()
        .chain(std::iter::once(next_word.clone()))
        .collect::<Vec<_>>();
    let candidate_text = join_word_units(&candidate_words);

    if count_words(&candidate_text) > max_segment_words_for_text(&candidate_text) {
        return false;
    }

    starts_lowercase(next_text)
        || is_terminal_text(next_text)
        || previous_text.trim_end().ends_with(',')
        || previous_text.trim_end().ends_with('，')
        || next_group.len() <= 2
}

fn needs_previous_completion(text: &str) -> bool {
    let trimmed = text.trim();
    if trimmed.is_empty() || is_terminal_text(trimmed) {
        return false;
    }

    let lower = trimmed.to_ascii_lowercase();
    let last_word = lower
        .split_whitespace()
        .last()
        .unwrap_or_default()
        .trim_matches(|character: char| !character.is_alphanumeric());

    trimmed.ends_with(',')
        || trimmed.ends_with('，')
        || matches!(
            last_word,
            "a" | "an"
                | "the"
                | "and"
                | "or"
                | "but"
                | "of"
                | "in"
                | "at"
                | "to"
                | "for"
                | "with"
                | "from"
                | "new"
                | "another"
                | "this"
                | "that"
        )
}

fn is_short_orphan_fragment(text: &str) -> bool {
    let trimmed = text.trim();
    if trimmed.is_empty() || is_terminal_text(trimmed) {
        return false;
    }

    if is_mainly_no_space_language(trimmed) {
        normalized_len(trimmed) <= 4
    } else {
        count_words(trimmed) <= 2
    }
}

fn is_terminal_text(text: &str) -> bool {
    let trimmed = text.trim_end();
    trimmed.ends_with('.')
        || trimmed.ends_with('!')
        || trimmed.ends_with('?')
        || trimmed.ends_with('。')
        || trimmed.ends_with('！')
        || trimmed.ends_with('？')
}

fn starts_lowercase(text: &str) -> bool {
    text.chars()
        .find(|character| character.is_alphabetic())
        .map(|character| character.is_lowercase())
        .unwrap_or(false)
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

fn parse_source_review_response(
    text: &str,
) -> Result<BTreeMap<String, ReviewedSourceEntry>, String> {
    let candidates = extract_json_object_candidates(text);
    if candidates.is_empty() {
        return Err("未找到 JSON 对象开始符".to_string());
    }

    let mut last_error = String::new();
    for json_text in candidates.iter().rev() {
        match parse_source_review_candidate(json_text) {
            Ok(result) => return Ok(result),
            Err(error) => last_error = error,
        }
    }

    Err(last_error)
}

fn parse_source_review_candidate(
    json_text: &str,
) -> Result<BTreeMap<String, ReviewedSourceEntry>, String> {
    let value = serde_json::from_str::<Value>(json_text)
        .map_err(|error| format!("JSON 解析失败: {error}"))?;
    parse_source_review_value(&value)
}

fn parse_source_review_value(
    value: &Value,
) -> Result<BTreeMap<String, ReviewedSourceEntry>, String> {
    let object = value
        .as_object()
        .ok_or_else(|| "LLM 返回内容不是 JSON 对象".to_string())?;

    if let Ok(result) = parse_source_review_object(object) {
        return Ok(result);
    }

    for field in [
        "reviewed", "reviews", "results", "result", "items", "data", "output",
    ] {
        let Some(nested) = object.get(field) else {
            continue;
        };

        match nested {
            Value::Object(nested_object) => {
                if let Ok(result) = parse_source_review_object(nested_object) {
                    return Ok(result);
                }
            }
            Value::Array(items) => {
                if let Ok(result) = parse_source_review_array(items) {
                    return Ok(result);
                }
            }
            _ => {}
        }
    }

    Err("未找到源文审核字幕编号 key".to_string())
}

fn parse_source_review_object(
    object: &serde_json::Map<String, Value>,
) -> Result<BTreeMap<String, ReviewedSourceEntry>, String> {
    let mut result = BTreeMap::new();
    let mut last_error = String::new();

    for (key, value) in object {
        let Some(normalized_key) = normalize_subtitle_key(key) else {
            continue;
        };

        match parse_reviewed_source_entry(value) {
            Ok(entry) => {
                result.insert(normalized_key, entry);
            }
            Err(error) => {
                last_error = format!("key {key} {error}");
            }
        }
    }

    if !result.is_empty() {
        return Ok(result);
    }

    if last_error.is_empty() {
        Err("未找到源文审核字幕编号 key".to_string())
    } else {
        Err(last_error)
    }
}

fn mark_unfinished_review_range(
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
        if segment.status == "reviewing" {
            segment.status = status.to_string();
        }
    }
}

fn parse_source_review_array(
    items: &[Value],
) -> Result<BTreeMap<String, ReviewedSourceEntry>, String> {
    let mut result = BTreeMap::new();
    let mut last_error = String::new();

    for item in items {
        let Some(object) = item.as_object() else {
            last_error = "数组项不是 JSON 对象".to_string();
            continue;
        };
        let Some(key) = extract_subtitle_key_from_object(object) else {
            last_error = "数组项缺少字幕编号字段".to_string();
            continue;
        };

        match parse_reviewed_source_entry(item) {
            Ok(entry) => {
                result.insert(key, entry);
            }
            Err(error) => {
                last_error = error;
            }
        }
    }

    if !result.is_empty() {
        return Ok(result);
    }

    if last_error.is_empty() {
        Err("未找到源文审核字幕编号 key".to_string())
    } else {
        Err(last_error)
    }
}

fn extract_subtitle_key_from_object(object: &serde_json::Map<String, Value>) -> Option<String> {
    ["id", "index", "key", "line", "line_number", "number"]
        .iter()
        .find_map(|field| {
            object.get(*field).and_then(|value| {
                value
                    .as_str()
                    .and_then(normalize_subtitle_key)
                    .or_else(|| value.as_u64().map(|number| number.to_string()))
            })
        })
}

fn normalize_subtitle_key(key: &str) -> Option<String> {
    let trimmed = key.trim().trim_start_matches('#');
    let numeric = trimmed.parse::<usize>().ok()?;

    if numeric == 0 {
        None
    } else {
        Some(numeric.to_string())
    }
}

fn parse_reviewed_source_entry(value: &Value) -> Result<ReviewedSourceEntry, String> {
    if let Some(text) = value.as_str() {
        return Ok(ReviewedSourceEntry {
            text: text.to_string(),
            action: "revise".to_string(),
        });
    }

    let object = value
        .as_object()
        .ok_or_else(|| "的值不是字符串或对象".to_string())?;
    let text = [
        "text",
        "source_text",
        "sourceText",
        "revised_text",
        "revisedText",
        "content",
        "value",
        "字幕",
    ]
    .iter()
    .find_map(|field| object.get(*field).and_then(Value::as_str))
    .unwrap_or_default()
    .to_string();
    let action = object
        .get("action")
        .or_else(|| object.get("operation"))
        .or_else(|| object.get("status"))
        .and_then(Value::as_str)
        .map(normalize_review_action)
        .unwrap_or_else(|| {
            if text.trim().is_empty() {
                "remove".to_string()
            } else {
                "revise".to_string()
            }
        });

    if !matches!(action.as_str(), "keep" | "revise" | "remove") {
        return Err("包含不支持的 action".to_string());
    }

    Ok(ReviewedSourceEntry { text, action })
}

fn normalize_review_action(action: &str) -> String {
    match action.trim().to_ascii_lowercase().as_str() {
        "remove" | "removed" | "delete" | "deleted" | "noise" | "drop" => "remove".to_string(),
        "keep" | "kept" | "unchanged" => "keep".to_string(),
        "revise" | "revised" | "edit" | "edited" | "fix" | "fixed" => "revise".to_string(),
        _ => action.trim().to_ascii_lowercase(),
    }
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

fn validate_source_review_keys(
    expected: &BTreeMap<String, String>,
    actual: &BTreeMap<String, ReviewedSourceEntry>,
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

fn validate_reference_correction_keys(
    expected: &BTreeMap<String, ReferenceCorrectionPromptEntry>,
    actual: &BTreeMap<String, ReviewedSourceEntry>,
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

#[cfg(test)]
mod tests {
    use super::*;

    fn test_segment(
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

    #[test]
    fn build_word_units_keeps_parent_segment_timing_bounds() {
        let segments = vec![test_segment(
            "你好世界",
            1000,
            3000,
            vec![
                TranscriptionWord {
                    text: "你好".to_string(),
                    start_time: 900,
                    end_time: 1200,
                },
                TranscriptionWord {
                    text: "世界".to_string(),
                    start_time: 1500,
                    end_time: 1900,
                },
            ],
        )];

        let words = build_word_units(&segments);

        assert_eq!(words.first().map(|word| word.start_time), Some(1000));
        assert_eq!(words.last().map(|word| word.end_time), Some(3000));
    }

    #[test]
    fn merge_word_units_uses_adjusted_parent_bounds() {
        let segments = vec![test_segment(
            "你好世界",
            1000,
            3000,
            vec![
                TranscriptionWord {
                    text: "你好".to_string(),
                    start_time: 900,
                    end_time: 1200,
                },
                TranscriptionWord {
                    text: "世界".to_string(),
                    start_time: 1500,
                    end_time: 1900,
                },
            ],
        )];
        let words = build_word_units(&segments);

        let merged = merge_word_units_by_sentences(&words, &["你好世界".to_string()]);

        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].start_time, 1000);
        assert_eq!(merged[0].end_time, 3000);
    }

    #[test]
    fn normalize_split_sentences_merges_short_orphan_fragment() {
        let segments = vec![
            test_segment(
                "Another day living in Guangzhou,",
                11_630,
                14_680,
                Vec::new(),
            ),
            test_segment("China", 14_680, 15_140, Vec::new()),
        ];
        let words = build_word_units(&segments);
        let source_text = join_word_units(&words);
        let sentences = vec![
            "Another day living in Guangzhou,".to_string(),
            "China".to_string(),
        ];

        let normalized = normalize_split_sentences_by_rules(&source_text, &words, &sentences)
            .expect("short orphan fragment should be repaired");

        assert_eq!(normalized, vec!["Another day living in Guangzhou, China"]);
    }
}
