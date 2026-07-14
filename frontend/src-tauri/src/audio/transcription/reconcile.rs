use crate::api::TranscriptSegment;
use std::collections::HashSet;

/// Removes likely echo transcripts captured by both the microphone and system
/// streams. It only acts on overlapping, highly similar cross-source segments,
/// and prefers the system-audio copy so distinct local speech is preserved.
pub fn suppress_cross_source_echoes(segments: Vec<TranscriptSegment>) -> Vec<TranscriptSegment> {
    let mut kept: Vec<TranscriptSegment> = Vec::with_capacity(segments.len());

    for segment in segments {
        let duplicate_index = kept.iter().position(|candidate| {
            cross_source(candidate, &segment)
                && time_overlap(candidate, &segment)
                && text_similarity(&candidate.text, &segment.text) >= 0.72
        });

        match duplicate_index {
            Some(index) if is_system_source(segment.source.as_deref()) => kept[index] = segment,
            Some(_) => {}
            None => kept.push(segment),
        }
    }

    kept
}

fn cross_source(left: &TranscriptSegment, right: &TranscriptSegment) -> bool {
    let left_source = left.source.as_deref().unwrap_or_default();
    let right_source = right.source.as_deref().unwrap_or_default();
    !left_source.eq_ignore_ascii_case(right_source)
        && ((is_system_source(Some(left_source)) && is_microphone_source(Some(right_source)))
            || (is_microphone_source(Some(left_source)) && is_system_source(Some(right_source))))
}

fn is_system_source(source: Option<&str>) -> bool {
    source.is_some_and(|value| value.eq_ignore_ascii_case("system"))
        || source.is_some_and(|value| value.eq_ignore_ascii_case("system audio"))
}

fn is_microphone_source(source: Option<&str>) -> bool {
    source.is_some_and(|value| value.eq_ignore_ascii_case("microphone"))
}

fn time_overlap(left: &TranscriptSegment, right: &TranscriptSegment) -> bool {
    match (
        left.audio_start_time,
        left.audio_end_time,
        right.audio_start_time,
        right.audio_end_time,
    ) {
        (Some(left_start), Some(left_end), Some(right_start), Some(right_end)) => {
            left_start <= right_end + 0.75 && right_start <= left_end + 0.75
        }
        _ => left.source_overlap.unwrap_or(false) || right.source_overlap.unwrap_or(false),
    }
}

fn text_similarity(left: &str, right: &str) -> f64 {
    let left_tokens = normalized_tokens(left);
    let right_tokens = normalized_tokens(right);
    if left_tokens.is_empty() || right_tokens.is_empty() {
        return 0.0;
    }
    let intersection = left_tokens.intersection(&right_tokens).count() as f64;
    let union = left_tokens.union(&right_tokens).count() as f64;
    intersection / union
}

fn normalized_tokens(text: &str) -> HashSet<String> {
    text.split_whitespace()
        .map(|token| {
            token
                .chars()
                .filter(|character| character.is_alphanumeric())
                .collect::<String>()
                .to_lowercase()
        })
        .filter(|token| !token.is_empty())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn segment(id: &str, source: &str, text: &str, start: f64, end: f64) -> TranscriptSegment {
        TranscriptSegment {
            id: id.to_string(),
            text: text.to_string(),
            timestamp: "10:00:00".to_string(),
            source: Some(source.to_string()),
            speaker: None,
            speaker_id: None,
            speaker_fingerprint: None,
            source_overlap: Some(true),
            audio_start_time: Some(start),
            audio_end_time: Some(end),
            duration: Some(end - start),
        }
    }

    #[test]
    fn keeps_system_copy_of_cross_source_echo() {
        let result = suppress_cross_source_echoes(vec![
            segment(
                "mic",
                "Microphone",
                "the launch budget was approved today",
                1.0,
                4.0,
            ),
            segment(
                "system",
                "System",
                "The launch budget was approved today.",
                1.2,
                4.2,
            ),
        ]);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].id, "system");
    }

    #[test]
    fn preserves_distinct_overlapping_speech() {
        let result = suppress_cross_source_echoes(vec![
            segment("mic", "Microphone", "I have a different question", 1.0, 4.0),
            segment(
                "system",
                "System",
                "The launch budget was approved",
                1.2,
                4.2,
            ),
        ]);
        assert_eq!(result.len(), 2);
    }
}
