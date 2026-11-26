use super::*;

impl ErgoNoteApp {
    pub(super) fn read_page_aloud(&mut self) {
        let Some(ed) = self.editor.as_ref() else {
            self.say(crate::tl_id!("ui.open_a_page_first"));
            return;
        };
        let mut collected: Vec<(Option<usize>, String, (usize, usize), (usize, usize))> = Vec::new();
        if let Some(open) = ed.across.filter(|a| !a.inside_one_block()) {
            for b_idx in open.blocks() {
                if let Some(block) = ed.blocks.get(b_idx) {
                    let whole = pageedit::block_text(block);
                    if let Some((ba, bb)) = open.covering(b_idx, whole.len()) {
                        let ca = whole.get(..ba).map_or(0, |s| s.chars().count());
                        let cb = whole.get(..bb).map_or_else(|| whole.chars().count(), |s| s.chars().count());
                        let prefix_chars = whole.chars().count().saturating_sub(pageedit::caret_text(block).chars().count());
                        let filter = (ca.saturating_sub(prefix_chars), cb.saturating_sub(prefix_chars));
                        if filter.0 < filter.1 {
                            for sw in pageedit::spoken_words_of_block(b_idx, block, None, Some(filter)) {
                                collected.push((Some(sw.block), sw.word, sw.caret_chars, sw.drawn_bytes));
                            }
                        }
                    }
                }
            }
        } else if let Some(c) = ed.caret.as_ref()
            && c.selection.0 != c.selection.1
            && let Some(block) = ed.blocks.get(c.block)
        {
            let (lo, hi) = (c.selection.0.min(c.selection.1), c.selection.0.max(c.selection.1));
            let active = c.cell.is_none().then_some(c.text.as_str());
            for sw in pageedit::spoken_words_of_block(c.block, block, active, Some((lo, hi))) {
                collected.push((Some(sw.block), sw.word, sw.caret_chars, sw.drawn_bytes));
            }
        }
        if collected.is_empty() {
            if !ed.title.trim().is_empty() {
                for raw in ed.title.split_whitespace() {
                    if raw.chars().any(char::is_alphanumeric) {
                        collected.push((None, raw.to_string(), (0, 0), (0, 0)));
                    }
                }
            }
            for (b_idx, b) in ed.blocks.iter().enumerate() {
                let active = ed.caret.as_ref().filter(|c| c.block == b_idx && c.cell.is_none()).map(|c| c.text.as_str());
                for sw in pageedit::spoken_words_of_block(b_idx, b, active, None) {
                    collected.push((Some(sw.block), sw.word, sw.caret_chars, sw.drawn_bytes));
                }
            }
        }
        let (clean, cues) = Self::build_cues_from_spoken_parts(&collected, self.tts_speed);
        self.speak_cues_with_speed(&clean, cues);
    }

    pub(super) fn read_active_line_aloud(&mut self) {
        let Some(ed) = self.editor.as_ref() else {
            self.say(crate::tl_id!("ui.open_a_page_first"));
            return;
        };
        let Some(c) = ed.caret.as_ref() else {
            self.say("Placez le curseur sur une phrase ou un paragraphe à lire.");
            return;
        };
        let Some(block) = ed.blocks.get(c.block) else {
            self.say("Placez le curseur sur une phrase ou un paragraphe à lire.");
            return;
        };
        let active = c.cell.is_none().then_some(c.text.as_str());
        let parts: Vec<(Option<usize>, String, (usize, usize), (usize, usize))> = pageedit::spoken_words_of_block(c.block, block, active, None)
            .into_iter()
            .map(|sw| (Some(sw.block), sw.word, sw.caret_chars, sw.drawn_bytes))
            .collect();
        if parts.is_empty() {
            self.say("Placez le curseur sur une phrase ou un paragraphe à lire.");
            return;
        }
        let (clean, cues) = Self::build_cues_from_spoken_parts(&parts, self.tts_speed);
        self.speak_cues_with_speed(&clean, cues);
    }

    pub(super) fn build_cues_from_spoken_parts(
        parts: &[(Option<usize>, String, (usize, usize), (usize, usize))],
        speed: TtsSpeed,
    ) -> (String, Vec<TtsWordCue>) {
        let base_wps = speed.words_per_second();
        let avg_word_dur = 1.0 / base_wps;
        let mut t = 0.0f32;
        let mut char_offset = 0usize;
        let mut clean = String::new();
        let mut cues = Vec::with_capacity(parts.len());
        for (idx, (block, raw, caret_chars, drawn_bytes)) in parts.iter().enumerate() {
            if idx > 0 {
                clean.push(' ');
                char_offset += 1;
            }
            let word_chars = raw.chars().count();
            let chars = raw.chars().filter(|c| c.is_alphanumeric()).count().max(1);
            let length_factor = (0.65 + (chars as f32) * 0.085).clamp(0.7, 1.9);
            let punct_pause = if raw.ends_with(['.', '!', '?', '…']) {
                0.28 / (base_wps / 2.45)
            } else if raw.ends_with([',', ';', ':']) {
                0.14 / (base_wps / 2.45)
            } else {
                0.0
            };
            let dur = avg_word_dur * length_factor;
            clean.push_str(raw);
            cues.push(TtsWordCue {
                word: raw.clone(),
                char_start: char_offset,
                start_sec: t,
                end_sec: t + dur,
                block: *block,
                caret_chars: *caret_chars,
                drawn_bytes: *drawn_bytes,
            });
            t += dur + punct_pause;
            char_offset += word_chars;
        }
        (clean, cues)
    }

    pub(super) fn build_tts_word_cues(text: &str, speed: TtsSpeed) -> Vec<TtsWordCue> {
        let base_wps = speed.words_per_second();
        let avg_word_dur = 1.0 / base_wps;
        let mut t = 0.0f32;
        let mut cues = Vec::new();
        let mut char_offset = 0usize;
        let mut rest = text;
        while !rest.is_empty() {
            let ws_bytes: usize = rest.chars().take_while(|c| c.is_whitespace()).map(char::len_utf8).sum();
            if ws_bytes > 0 {
                char_offset += rest[..ws_bytes].chars().count();
                rest = &rest[ws_bytes..];
                continue;
            }
            let word_bytes: usize = rest.chars().take_while(|c| !c.is_whitespace()).map(char::len_utf8).sum();
            let raw = &rest[..word_bytes];
            let word_chars = raw.chars().count();
            let chars = raw.chars().filter(|c| c.is_alphanumeric()).count().max(1);
            let length_factor = (0.65 + (chars as f32) * 0.085).clamp(0.7, 1.9);
            let punct_pause = if raw.ends_with(['.', '!', '?', '…']) {
                0.28 / (base_wps / 2.45)
            } else if raw.ends_with([',', ';', ':']) {
                0.14 / (base_wps / 2.45)
            } else {
                0.0
            };
            let dur = avg_word_dur * length_factor;
            cues.push(TtsWordCue {
                word: raw.to_string(),
                char_start: char_offset,
                start_sec: t,
                end_sec: t + dur,
                block: None,
                caret_chars: (0, 0),
                drawn_bytes: (0, 0),
            });
            t += dur + punct_pause;
            char_offset += word_chars;
            rest = &rest[word_bytes..];
        }
        cues
    }

    #[cfg_attr(target_os = "android", allow(dead_code))]
    pub(super) fn alnum_norm(s: &str) -> String {
        s.chars().filter(|c| c.is_alphanumeric()).flat_map(char::to_lowercase).collect()
    }

    /// Maps millisecond-accurate `WordBoundary` events (`start\tend\tword` TSV) onto existing
    /// `TtsWordCue`s (preserving block/caret/drawn coordinates for page highlighting).
    #[cfg_attr(target_os = "android", allow(dead_code))]
    pub(super) fn apply_word_boundary_tsv(cues: &mut [TtsWordCue], tsv: &str, audio_offset_sec: f32) -> bool {
        let mut wb: Vec<(f32, f32, String)> = Vec::new();
        for line in tsv.lines() {
            let mut cols = line.splitn(3, '\t');
            if let (Some(s_str), Some(e_str), Some(txt)) = (cols.next(), cols.next(), cols.next())
                && let (Ok(s), Ok(e)) = (s_str.trim().parse::<f32>(), e_str.trim().parse::<f32>())
            {
                let norm = Self::alnum_norm(txt);
                if !norm.is_empty() && e >= s {
                    wb.push((s, e, norm));
                }
            }
        }
        if wb.is_empty() || cues.is_empty() {
            return false;
        }
        let mut wi = 0usize;
        let mut assigned = 0usize;
        let mut last_end = audio_offset_sec;
        for cue in cues.iter_mut() {
            let target = Self::alnum_norm(&cue.word);
            if target.is_empty() || wi >= wb.len() {
                cue.start_sec = last_end;
                cue.end_sec = last_end + 0.05;
                last_end = cue.end_sec;
                continue;
            }
            if wb[wi].2 == target {
                cue.start_sec = wb[wi].0 + audio_offset_sec;
                cue.end_sec = wb[wi].1.max(wb[wi].0 + 0.04) + audio_offset_sec;
                last_end = cue.end_sec;
                wi += 1;
                assigned += 1;
                continue;
            }
            // Handle apostrophe/hyphen splits across consecutive WordBoundary events.
            if wi + 1 < wb.len() {
                let mut combined = wb[wi].2.clone();
                let mut end_t = wb[wi].1;
                let mut merged = None;
                for ahead in 1..=3.min(wb.len() - wi - 1) {
                    combined.push_str(&wb[wi + ahead].2);
                    end_t = wb[wi + ahead].1;
                    if combined == target {
                        merged = Some(ahead + 1);
                        break;
                    }
                }
                if let Some(advance) = merged {
                    cue.start_sec = wb[wi].0 + audio_offset_sec;
                    cue.end_sec = end_t.max(wb[wi].0 + 0.04) + audio_offset_sec;
                    last_end = cue.end_sec;
                    wi += advance;
                    assigned += 1;
                    continue;
                }
            }
            // Look ahead slightly in case TTS inserted/skipped a non-lexical symbol.
            if let Some(rel) = (1..=3.min(wb.len() - wi - 1)).find(|&k| wb[wi + k].2 == target) {
                wi += rel;
                cue.start_sec = wb[wi].0 + audio_offset_sec;
                cue.end_sec = wb[wi].1.max(wb[wi].0 + 0.04) + audio_offset_sec;
                last_end = cue.end_sec;
                wi += 1;
                assigned += 1;
            } else {
                cue.start_sec = wb[wi].0 + audio_offset_sec;
                cue.end_sec = wb[wi].1.max(wb[wi].0 + 0.04) + audio_offset_sec;
                last_end = cue.end_sec;
                wi += 1;
                assigned += 1;
            }
        }
        assigned > 0
    }

    #[cfg_attr(any(test, not(target_os = "linux")), allow(dead_code))]
    pub(super) fn edge_tts_python_interpreter(edge_cmd: &str) -> String {
        if let Ok(raw) = std::fs::read_to_string(edge_cmd)
            && let Some(first_line) = raw.lines().next()
            && let Some(interp) = first_line.strip_prefix("#!")
        {
            let trimmed = interp.trim();
            if !trimmed.is_empty() && !trimmed.contains(' ') {
                return trimmed.to_owned();
            }
        }
        "python3".to_owned()
    }

    /// Parses a standard RIFF WAVE file in pure Rust and returns its exact audio duration in seconds.
    #[cfg_attr(target_os = "android", allow(dead_code))]
    pub(super) fn wav_duration_secs(bytes: &[u8]) -> Option<f32> {
        if bytes.len() < 12 || &bytes[0..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
            return None;
        }
        let mut byte_rate = 0u32;
        let mut data_bytes = 0u32;
        let mut pos = 12usize;
        while pos + 8 <= bytes.len() {
            let id = &bytes[pos..pos + 4];
            let chunk_size = u32::from_le_bytes([bytes[pos + 4], bytes[pos + 5], bytes[pos + 6], bytes[pos + 7]]);
            let payload_start = pos + 8;
            if id == b"fmt " && chunk_size >= 16 && payload_start + 12 <= bytes.len() {
                byte_rate =
                    u32::from_le_bytes([bytes[payload_start + 8], bytes[payload_start + 9], bytes[payload_start + 10], bytes[payload_start + 11]]);
            } else if id == b"data" {
                let avail = bytes.len().saturating_sub(payload_start);
                data_bytes = chunk_size.min(u32::try_from(avail).unwrap_or(u32::MAX));
            }
            let padded = usize::try_from(chunk_size).ok()?.saturating_add(usize::from(!chunk_size.is_multiple_of(2)));
            pos = payload_start.checked_add(padded)?;
        }
        (byte_rate > 0 && data_bytes > 0).then(|| data_bytes as f32 / byte_rate as f32)
    }

    /// Estimates exact playback duration for `edge-tts` 48 kbps CBR mono MP3 output (`6000 B/s`).
    #[cfg_attr(not(target_os = "linux"), allow(dead_code))]
    pub(super) fn mp3_duration_secs(bytes: &[u8]) -> Option<f32> {
        (bytes.len() >= 128).then(|| bytes.len() as f32 / 6000.0)
    }

    #[cfg_attr(target_os = "android", allow(dead_code))]
    pub(super) fn rescale_tts_cues(cues: &mut [TtsWordCue], actual_total_sec: f32, audio_offset_sec: f32) {
        let Some(orig_total) = cues.last().map(|c| c.end_sec) else { return };
        if actual_total_sec <= 0.05 || orig_total <= 0.001 {
            return;
        }
        let scale = actual_total_sec / orig_total;
        for cue in cues {
            cue.start_sec = audio_offset_sec + cue.start_sec * scale;
            cue.end_sec = audio_offset_sec + cue.end_sec * scale;
        }
    }

    #[cfg_attr(target_os = "android", allow(dead_code))]
    pub(super) fn parse_vtt_timestamp_secs(ts: &str) -> Option<f32> {
        let clean = ts.split_whitespace().next()?;
        let mut parts: Vec<&str> = clean.split(':').collect();
        let sec_str = parts.pop()?.replace(',', ".");
        let secs: f32 = sec_str.parse().ok()?;
        let mins: f32 = parts.pop().and_then(|m| m.parse().ok()).unwrap_or(0.0);
        let hours: f32 = parts.pop().and_then(|h| h.parse().ok()).unwrap_or(0.0);
        Some(hours * 3600.0 + mins * 60.0 + secs)
    }

    /// Re-aligns existing word cues onto WebVTT sentence intervals while keeping page coordinates.
    #[cfg_attr(target_os = "android", allow(dead_code))]
    pub(super) fn apply_vtt_to_cues(vtt: &str, cues: &mut [TtsWordCue], audio_offset_sec: f32) -> bool {
        if cues.is_empty() {
            return false;
        }
        let mut intervals: Vec<(f32, f32, usize)> = Vec::new();
        let lines: Vec<&str> = vtt.lines().collect();
        let mut idx = 0usize;
        while idx < lines.len() {
            let line = lines[idx].trim();
            if let Some((start_s, end_s)) = line.split_once("-->")
                && let (Some(start), Some(end)) = (Self::parse_vtt_timestamp_secs(start_s), Self::parse_vtt_timestamp_secs(end_s))
            {
                idx += 1;
                let mut words_in_cue = 0usize;
                while idx < lines.len() && !lines[idx].trim().is_empty() {
                    words_in_cue += lines[idx].split_whitespace().count();
                    idx += 1;
                }
                if end > start && words_in_cue > 0 {
                    intervals.push((start, end, words_in_cue));
                }
            } else {
                idx += 1;
            }
        }
        if intervals.is_empty() {
            return false;
        }
        let mut word_cursor = 0usize;
        let last_seg = intervals.len() - 1;
        for (seg_idx, (start_sec, end_sec, word_count)) in intervals.into_iter().enumerate() {
            if word_cursor >= cues.len() {
                break;
            }
            let take_end = if seg_idx == last_seg { cues.len() } else { (word_cursor + word_count).min(cues.len()) };
            if take_end <= word_cursor {
                continue;
            }
            let span_dur = (end_sec - start_sec).max(0.08);
            let slice = &mut cues[word_cursor..take_end];
            let weights: Vec<f32> = slice
                .iter()
                .enumerate()
                .map(|(k, c)| {
                    let chars = c.word.chars().filter(|ch| ch.is_alphanumeric()).count().max(1);
                    let w = (0.65 + chars as f32 * 0.085).clamp(0.7, 1.9);
                    let pause = if k + 1 < slice.len() && c.word.ends_with([',', ';', ':', '.']) { 0.45 } else { 0.0 };
                    w + pause
                })
                .collect();
            let sum_w: f32 = weights.iter().sum::<f32>().max(0.01);
            let mut t = start_sec + audio_offset_sec;
            for (cue, weight) in slice.iter_mut().zip(weights) {
                let dur = span_dur * (weight / sum_w);
                cue.start_sec = t;
                cue.end_sec = t + dur;
                t += dur;
            }
            word_cursor = take_end;
        }
        true
    }

    #[cfg(test)]
    pub(super) fn parse_vtt_word_cues(vtt: &str, full_text: &str, speed: TtsSpeed, audio_offset_sec: f32) -> Option<Vec<TtsWordCue>> {
        let mut cues = Self::build_tts_word_cues(full_text, speed);
        Self::apply_vtt_to_cues(vtt, &mut cues, audio_offset_sec).then_some(cues)
    }

    pub(super) fn speak_text_with_speed(&mut self, text: &str) {
        let cues = Self::build_tts_word_cues(text, self.tts_speed);
        self.speak_cues_with_speed(text, cues);
    }

    pub(super) fn speak_cues_with_speed(&mut self, text: &str, initial_cues: Vec<TtsWordCue>) {
        let clean = text.trim().to_string();
        if clean.is_empty() {
            self.say("Aucun texte à lire sur cette page.");
            return;
        }
        self.stop_tts();
        self.tts_done.store(false, std::sync::atomic::Ordering::SeqCst);
        self.tts_stop_flag.store(false, std::sync::atomic::Ordering::SeqCst);
        self.tts_paused_flag.store(false, std::sync::atomic::Ordering::SeqCst);
        self.tts_state = TtsState::Playing;
        self.tts_last_text = clean.clone();
        self.tts_words = initial_cues.clone();
        if let Ok(mut sync) = self.tts_sync.lock() {
            *sync = TtsSync {
                preparing: cfg!(any(target_os = "linux", target_os = "windows")),
                playback_started_at: if cfg!(any(target_os = "linux", target_os = "windows")) { None } else { Some(Instant::now()) },
                elapsed_before_pause: 0.0,
                cues: initial_cues.clone(),
                direct_word_index: None,
            };
        }
        self.pending_speech = Some(clean.clone());
        let speed = self.tts_speed;
        let lang = self.spell_lang;
        let pid_slot = self.tts_pid.clone();
        let done_slot = self.tts_done.clone();
        let stop_flag = self.tts_stop_flag.clone();
        #[cfg_attr(all(target_os = "windows", not(test)), allow(unused_variables))]
        let paused_flag = self.tts_paused_flag.clone();
        let sync_arc = self.tts_sync.clone();
        #[cfg(all(target_os = "windows", not(test)))]
        {
            let win_rate = speed.win_rate();
            let edge_voice = lang.edge_voice();
            let edge_rate = speed.edge_rate();
            let piper_file = lang.piper_model_name();
            let piper_scale = speed.piper_length_scale();
            crate::tts_win::ensure_piper_in_background(piper_file);
            std::thread::spawn(move || {
                use std::io::{BufRead, BufReader};
                use std::os::windows::process::CommandExt;
                use std::sync::atomic::Ordering;
                const CREATE_NO_WINDOW: u32 = 0x0800_0000;
                let wait_unpaused = || -> bool {
                    while paused_flag.load(Ordering::SeqCst) {
                        if stop_flag.load(Ordering::SeqCst) {
                            return false;
                        }
                        std::thread::sleep(std::time::Duration::from_millis(30));
                    }
                    !stop_flag.load(Ordering::SeqCst)
                };
                let begin_clock = |updated_cues: Option<Vec<TtsWordCue>>| {
                    if let Ok(mut sync) = sync_arc.lock() {
                        if let Some(cues) = updated_cues {
                            sync.cues = cues;
                        }
                        sync.preparing = false;
                        sync.elapsed_before_pause = 0.0;
                        sync.playback_started_at = Some(Instant::now());
                    }
                };
                let tmp = std::env::temp_dir();
                let mp3 = tmp.join("ergonote_tts.mp3");
                let tsv = tmp.join("ergonote_tts.words.tsv");
                let wav = tmp.join("ergonote_tts.wav");

                // 1) Priority 1: online Edge Neural voice with millisecond WordBoundary sync.
                if crate::tts_win::synthesize_edge_mp3(edge_voice, edge_rate, &clean, (&mp3, &tsv), &pid_slot, &stop_flag) && wait_unpaused() {
                    let mut cues = initial_cues.clone();
                    let wb_ok = std::fs::read_to_string(&tsv).ok().is_some_and(|txt| Self::apply_word_boundary_tsv(&mut cues, &txt, 0.10));
                    if !wb_ok
                        && let Ok(bytes) = std::fs::read(&mp3)
                        && let Some(dur) = Self::mp3_duration_secs(&bytes)
                    {
                        Self::rescale_tts_cues(&mut cues, dur, 0.12);
                    }
                    begin_clock(Some(cues));
                    let _ = crate::tts_win::play_audio(&mp3, false, &pid_slot, &stop_flag);
                    done_slot.store(true, Ordering::SeqCst);
                    return;
                }

                // 2) Priority 2: cached offline Piper neural voice synced against WAV duration.
                if crate::tts_win::synthesize_piper_wav(piper_file, piper_scale, &clean, &wav, &pid_slot, &stop_flag) && wait_unpaused() {
                    let synced = std::fs::read(&wav).ok().and_then(|bytes| Self::wav_duration_secs(&bytes)).map(|dur| {
                        let mut cues = initial_cues.clone();
                        Self::rescale_tts_cues(&mut cues, dur, 0.06);
                        cues
                    });
                    begin_clock(synced);
                    let _ = crate::tts_win::play_audio(&wav, true, &pid_slot, &stop_flag);
                    done_slot.store(true, Ordering::SeqCst);
                    return;
                }

                // 3) Last-resort fallback: built-in Windows SAPI System.Speech.
                if !wait_unpaused() {
                    done_slot.store(true, Ordering::SeqCst);
                    return;
                }
                let cmd = format!(
                    "Add-Type -AssemblyName System.Speech; $s = New-Object System.Speech.Synthesis.SpeechSynthesizer; $s.Rate = {win_rate}; $s.add_SpeakProgress({{ param($sender, $e) [Console]::Out.WriteLine('AT:' + $e.CharacterPosition); [Console]::Out.Flush() }}); $s.Speak($env:ERGONOTE_TTS)"
                );
                if let Ok(mut child) = std::process::Command::new("powershell")
                    .args(["-NoProfile", "-NonInteractive", "-WindowStyle", "Hidden", "-Command", &cmd])
                    .env("ERGONOTE_TTS", &clean)
                    .stdout(std::process::Stdio::piped())
                    .creation_flags(CREATE_NO_WINDOW)
                    .spawn()
                {
                    pid_slot.store(child.id(), Ordering::SeqCst);
                    if let Ok(mut sync) = sync_arc.lock() {
                        sync.preparing = false;
                        sync.playback_started_at = Some(Instant::now());
                        sync.direct_word_index = Some(0);
                    }
                    if let Some(stdout) = child.stdout.take() {
                        let reader = BufReader::new(stdout);
                        for line in reader.lines().map_while(Result::ok) {
                            if stop_flag.load(Ordering::SeqCst) {
                                break;
                            }
                            if let Some(rest) = line.trim().strip_prefix("AT:")
                                && let Ok(char_pos) = rest.parse::<usize>()
                                && let Ok(mut sync) = sync_arc.lock()
                            {
                                sync.direct_word_index = sync.cues.iter().rposition(|c| c.char_start <= char_pos).or(Some(0));
                            }
                        }
                    }
                    let _ = child.wait();
                }
                pid_slot.store(0, Ordering::SeqCst);
                done_slot.store(true, Ordering::SeqCst);
            });
        }
        #[cfg(all(target_os = "linux", not(test)))]
        {
            let edge_voice = lang.edge_voice();
            let edge_rate = speed.edge_rate();
            let piper_file = lang.piper_model_name();
            let piper_scale = speed.piper_length_scale();
            let pico_locale = lang.tts_locale();
            let espeak_voice = lang.espeak_voice();
            let spd_rate = speed.spd_rate();
            let espeak_wpm = speed.espeak_wpm();
            std::thread::spawn(move || {
                use std::io::Write;
                use std::sync::atomic::Ordering;
                let home = std::env::var("HOME").unwrap_or_default();
                let mp3 = "/tmp/ergonote_tts.mp3";
                let vtt = "/tmp/ergonote_tts.vtt";
                let tsv = "/tmp/ergonote_tts.words.tsv";
                let wav = "/tmp/ergonote_tts.wav";
                let wait_unpaused = || -> bool {
                    while paused_flag.load(Ordering::SeqCst) {
                        if stop_flag.load(Ordering::SeqCst) {
                            return false;
                        }
                        std::thread::sleep(std::time::Duration::from_millis(30));
                    }
                    !stop_flag.load(Ordering::SeqCst)
                };
                let begin_clock = |updated_cues: Option<Vec<TtsWordCue>>| {
                    if let Ok(mut sync) = sync_arc.lock() {
                        if let Some(cues) = updated_cues {
                            sync.cues = cues;
                        }
                        sync.preparing = false;
                        sync.elapsed_before_pause = 0.0;
                        sync.playback_started_at = Some(Instant::now());
                    }
                };
                let run_tracked = |mut cmd: std::process::Command| -> bool {
                    if stop_flag.load(Ordering::SeqCst) {
                        return false;
                    }
                    if let Ok(mut child) = cmd.spawn() {
                        pid_slot.store(child.id(), Ordering::SeqCst);
                        let ok = child.wait().is_ok_and(|s| s.success());
                        pid_slot.store(0, Ordering::SeqCst);
                        ok
                    } else {
                        false
                    }
                };

                // 1a) Edge Neural voice via Python WordBoundary stream for millisecond per-word timestamps.
                let edge_bin = format!("{home}/.local/bin/edge-tts");
                let edge_cmd = if std::path::Path::new(&edge_bin).is_file() { edge_bin.as_str() } else { "edge-tts" };
                let py = Self::edge_tts_python_interpreter(edge_cmd);
                let _ = std::fs::remove_file(tsv);
                let _ = std::fs::remove_file(vtt);
                let py_script = "\
import asyncio, os, edge_tts
async def main():
    c = edge_tts.Communicate(os.environ['ERGONOTE_TTS'], os.environ['ERGONOTE_TTS_VOICE'], rate=os.environ['ERGONOTE_TTS_RATE'], boundary='WordBoundary')
    with open(os.environ['ERGONOTE_TTS_MP3'], 'wb') as f, open(os.environ['ERGONOTE_TTS_TSV'], 'w', encoding='utf-8') as w:
        async for chunk in c.stream():
            if chunk['type'] == 'audio':
                f.write(chunk['data'])
            elif chunk['type'] == 'WordBoundary':
                s = chunk['offset'] / 10_000_000.0
                d = chunk['duration'] / 10_000_000.0
                w.write(f\"{s:.4f}\\t{s + d:.4f}\\t{chunk['text']}\\n\")
asyncio.run(main())";
                let mut edge_py = std::process::Command::new(&py);
                edge_py.args(["-c", py_script]);
                edge_py.env("ERGONOTE_TTS", &clean);
                edge_py.env("ERGONOTE_TTS_VOICE", edge_voice);
                edge_py.env("ERGONOTE_TTS_RATE", edge_rate);
                edge_py.env("ERGONOTE_TTS_MP3", mp3);
                edge_py.env("ERGONOTE_TTS_TSV", tsv);
                let mut edge_ok = run_tracked(edge_py);
                if !edge_ok && !stop_flag.load(Ordering::SeqCst) {
                    // 1b) CLI fallback with VTT sentence intervals.
                    let mut edge = std::process::Command::new(edge_cmd);
                    let rate_arg = format!("--rate={edge_rate}");
                    edge.args(["--voice", edge_voice, &rate_arg, "--text", &clean, "--write-media", mp3, "--write-subtitles", vtt]);
                    edge_ok = run_tracked(edge);
                }
                if edge_ok && wait_unpaused() {
                    let mut cues = initial_cues.clone();
                    let wb_ok = std::fs::read_to_string(tsv).ok().is_some_and(|txt| Self::apply_word_boundary_tsv(&mut cues, &txt, 0.10));
                    if !wb_ok {
                        let vtt_ok = std::fs::read_to_string(vtt).ok().is_some_and(|txt| Self::apply_vtt_to_cues(&txt, &mut cues, 0.12));
                        if !vtt_ok
                            && let Ok(bytes) = std::fs::read(mp3)
                            && let Some(dur) = Self::mp3_duration_secs(&bytes)
                        {
                            Self::rescale_tts_cues(&mut cues, dur, 0.12);
                        }
                    }
                    begin_clock(Some(cues));
                    let mut play = std::process::Command::new("gst-play-1.0");
                    play.args(["--no-interactive", "--quiet", mp3]);
                    let _ = run_tracked(play);
                    done_slot.store(true, Ordering::SeqCst);
                    return;
                }

                // 2) Offline fallback: local Piper neural voice synced against actual WAV duration.
                let piper_bin = format!("{home}/.local/share/ergonote-tts/piper/piper");
                let piper_model = format!("{home}/.local/share/ergonote-tts/{piper_file}");
                if !stop_flag.load(Ordering::SeqCst)
                    && std::path::Path::new(&piper_bin).is_file()
                    && std::path::Path::new(&piper_model).is_file()
                    && let Ok(mut child) = std::process::Command::new(&piper_bin)
                        .args(["--model", &piper_model, "--length_scale", piper_scale, "--output_file", wav])
                        .stdin(std::process::Stdio::piped())
                        .stdout(std::process::Stdio::null())
                        .stderr(std::process::Stdio::null())
                        .spawn()
                {
                    if let Some(mut stdin) = child.stdin.take() {
                        let _ = stdin.write_all(clean.as_bytes());
                    }
                    if child.wait().is_ok_and(|s| s.success()) && wait_unpaused() {
                        let synced = std::fs::read(wav).ok().and_then(|bytes| Self::wav_duration_secs(&bytes)).map(|dur| {
                            let mut cues = initial_cues.clone();
                            Self::rescale_tts_cues(&mut cues, dur, 0.06);
                            cues
                        });
                        begin_clock(synced);
                        let mut play = std::process::Command::new("paplay");
                        play.arg(wav);
                        if !run_tracked(play) && !stop_flag.load(Ordering::SeqCst) {
                            let mut play2 = std::process::Command::new("aplay");
                            play2.arg(wav);
                            let _ = run_tracked(play2);
                        }
                        done_slot.store(true, Ordering::SeqCst);
                        return;
                    }
                }

                // 3) Offline fallback: SVOX Pico voice (pico2wave) synced against WAV duration.
                if !stop_flag.load(Ordering::SeqCst) {
                    let pico_ok =
                        std::process::Command::new("pico2wave").args(["-l", pico_locale, "-w", wav, &clean]).status().is_ok_and(|s| s.success());
                    if pico_ok && wait_unpaused() {
                        let synced = std::fs::read(wav).ok().and_then(|bytes| Self::wav_duration_secs(&bytes)).map(|dur| {
                            let mut cues = initial_cues.clone();
                            Self::rescale_tts_cues(&mut cues, dur, 0.06);
                            cues
                        });
                        begin_clock(synced);
                        let mut play = std::process::Command::new("paplay");
                        play.arg(wav);
                        if !run_tracked(play) && !stop_flag.load(Ordering::SeqCst) {
                            let mut play2 = std::process::Command::new("aplay");
                            play2.arg(wav);
                            let _ = run_tracked(play2);
                        }
                        done_slot.store(true, Ordering::SeqCst);
                        return;
                    }
                }

                // 4) Last resort fallback (`spd-say` / `espeak-ng`).
                if wait_unpaused() {
                    begin_clock(None);
                    let mut spd = std::process::Command::new("spd-say");
                    spd.args(["-l", espeak_voice, "-r", spd_rate, "--", &clean]);
                    if !run_tracked(spd) && !stop_flag.load(Ordering::SeqCst) {
                        let mut espeak = std::process::Command::new("espeak-ng");
                        espeak.args(["-v", espeak_voice, "-s", espeak_wpm, "--", &clean]);
                        let _ = run_tracked(espeak);
                    }
                }
                done_slot.store(true, Ordering::SeqCst);
            });
        }
        #[cfg(all(target_os = "macos", not(test)))]
        {
            let say_wpm = speed.say_wpm();
            let _ = (&lang, &sync_arc, &paused_flag, &initial_cues);
            std::thread::spawn(move || {
                if let Ok(mut child) = std::process::Command::new("say").args(["-r", say_wpm, &clean]).spawn() {
                    pid_slot.store(child.id(), std::sync::atomic::Ordering::SeqCst);
                    let _ = child.wait();
                }
                pid_slot.store(0, std::sync::atomic::Ordering::SeqCst);
                done_slot.store(true, std::sync::atomic::Ordering::SeqCst);
            });
        }
        #[cfg(all(target_os = "android", not(test)))]
        {
            let _ = (&clean, speed, lang, pid_slot, done_slot, stop_flag, paused_flag, initial_cues);
            if let Ok(mut sync) = sync_arc.lock() {
                sync.preparing = false;
                sync.elapsed_before_pause = 0.0;
                sync.playback_started_at = Some(Instant::now());
            }
        }
        #[cfg(any(test, target_arch = "wasm32", target_os = "ios"))]
        {
            let _ = (&clean, speed, lang, pid_slot, done_slot, stop_flag, paused_flag, sync_arc, initial_cues);
        }
        self.say(format!("Lecture à voix haute ({}) lancée.", self.tts_speed.label()));
    }

    pub(super) fn stop_word_preview(&mut self) {
        let pid = self.word_preview_pid.swap(0, std::sync::atomic::Ordering::SeqCst);
        #[cfg(all(unix, not(test)))]
        if pid > 0 {
            let _ = std::process::Command::new("kill").args(["-TERM", &pid.to_string()]).status();
        }
        #[cfg(all(target_os = "windows", not(test)))]
        if pid > 0 {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            let _ = std::process::Command::new("taskkill").args(["/PID", &pid.to_string(), "/T", "/F"]).creation_flags(CREATE_NO_WINDOW).status();
        }
        #[cfg(any(test, target_arch = "wasm32", target_os = "android"))]
        let _ = pid;
    }

    pub(super) fn speak_word_preview(&mut self, word: &str) {
        let word = word.trim().to_string();
        if word.is_empty() {
            return;
        }
        self.stop_word_preview();
        self.word_preview_seq = self.word_preview_seq.wrapping_add(1);
        let seq = self.word_preview_seq;
        let lang = self.spell_lang;
        let speed = self.tts_speed;
        let preview_pid = self.word_preview_pid.clone();
        self.pending_speech = Some(word.clone());
        #[cfg(all(target_os = "windows", not(test)))]
        {
            let win_rate = speed.win_rate();
            let edge_voice = lang.edge_voice();
            let piper_file = lang.piper_model_name();
            let piper_scale = speed.piper_length_scale();
            let _ = seq;
            crate::tts_win::ensure_piper_in_background(piper_file);
            std::thread::spawn(move || {
                use std::os::windows::process::CommandExt;
                use std::sync::atomic::{AtomicBool, Ordering};
                const CREATE_NO_WINDOW: u32 = 0x0800_0000;
                let never_stop = AtomicBool::new(false);
                let tmp = std::env::temp_dir();
                let wav = tmp.join("ergonote_word.wav");
                let mp3 = tmp.join("ergonote_word.mp3");
                let tsv = tmp.join("ergonote_word.tsv");
                if crate::tts_win::synthesize_piper_wav(piper_file, piper_scale, &word, &wav, &preview_pid, &never_stop)
                    && crate::tts_win::play_audio(&wav, true, &preview_pid, &never_stop)
                {
                    return;
                }
                if crate::tts_win::synthesize_edge_mp3(edge_voice, "+0%", &word, (&mp3, &tsv), &preview_pid, &never_stop)
                    && crate::tts_win::play_audio(&mp3, false, &preview_pid, &never_stop)
                {
                    return;
                }
                let cmd = format!(
                    "Add-Type -AssemblyName System.Speech; $s = New-Object System.Speech.Synthesis.SpeechSynthesizer; $s.Rate = {win_rate}; $s.Speak($env:ERGONOTE_WORD)"
                );
                if let Ok(mut child) = std::process::Command::new("powershell")
                    .args(["-NoProfile", "-NonInteractive", "-WindowStyle", "Hidden", "-Command", &cmd])
                    .env("ERGONOTE_WORD", &word)
                    .creation_flags(CREATE_NO_WINDOW)
                    .spawn()
                {
                    preview_pid.store(child.id(), Ordering::SeqCst);
                    let _ = child.wait();
                }
                preview_pid.store(0, Ordering::SeqCst);
            });
        }
        #[cfg(all(target_os = "linux", not(test)))]
        {
            let home = std::env::var("HOME").unwrap_or_default();
            let piper_file = lang.piper_model_name();
            let piper_scale = speed.piper_length_scale();
            let edge_voice = lang.edge_voice();
            let pico_locale = lang.tts_locale();
            let espeak_voice = lang.espeak_voice();
            let espeak_wpm = speed.espeak_wpm();
            std::thread::spawn(move || {
                use std::io::Write;
                use std::sync::atomic::Ordering;
                let proc_id = std::process::id();
                let wav = std::env::temp_dir().join(format!("ergonote_word_{proc_id}_{seq}.wav"));
                let mp3 = std::env::temp_dir().join(format!("ergonote_word_{proc_id}_{seq}.mp3"));
                let play_tracked = |mut cmd: std::process::Command| -> bool {
                    if let Ok(mut child) = cmd.spawn() {
                        preview_pid.store(child.id(), Ordering::SeqCst);
                        let ok = child.wait().is_ok_and(|s| s.success());
                        preview_pid.store(0, Ordering::SeqCst);
                        ok
                    } else {
                        false
                    }
                };

                // 1) Local neural Piper voice when installed for the active language (natural voice in ~0.4s offline).
                let piper_bin = format!("{home}/.local/share/ergonote-tts/piper/piper");
                let piper_model = format!("{home}/.local/share/ergonote-tts/{piper_file}");
                if std::path::Path::new(&piper_bin).is_file()
                    && std::path::Path::new(&piper_model).is_file()
                    && let Ok(mut child) = std::process::Command::new(&piper_bin)
                        .args(["--model", &piper_model, "--length_scale", piper_scale, "--output_file"])
                        .arg(&wav)
                        .stdin(std::process::Stdio::piped())
                        .stdout(std::process::Stdio::null())
                        .stderr(std::process::Stdio::null())
                        .spawn()
                {
                    if let Some(mut stdin) = child.stdin.take() {
                        let _ = stdin.write_all(word.as_bytes());
                    }
                    if child.wait().is_ok_and(|s| s.success()) {
                        let mut play = std::process::Command::new("paplay");
                        play.arg(&wav);
                        if !play_tracked(play) {
                            let mut play2 = std::process::Command::new("aplay");
                            play2.arg(&wav);
                            let _ = play_tracked(play2);
                        }
                        let _ = std::fs::remove_file(&wav);
                        return;
                    }
                }

                // 2) Edge neural voice if available (ideal for EN/DE/ES/IT when Piper model isn't cached locally).
                let edge_bin = format!("{home}/.local/bin/edge-tts");
                let edge_cmd = if std::path::Path::new(&edge_bin).is_file() { edge_bin.as_str() } else { "edge-tts" };
                if std::process::Command::new(edge_cmd)
                    .args(["--voice", edge_voice, "--text", &word, "--write-media"])
                    .arg(&mp3)
                    .status()
                    .is_ok_and(|s| s.success())
                {
                    let mut play = std::process::Command::new("gst-play-1.0");
                    play.args(["--no-interactive", "--quiet"]);
                    play.arg(&mp3);
                    let _ = play_tracked(play);
                    let _ = std::fs::remove_file(&mp3);
                    return;
                }

                // 3) Offline SVOX Pico fallback, then spd-say / espeak-ng.
                if std::process::Command::new("pico2wave").args(["-l", pico_locale, "-w"]).arg(&wav).arg(&word).status().is_ok_and(|s| s.success()) {
                    let mut play = std::process::Command::new("paplay");
                    play.arg(&wav);
                    let ok = play_tracked(play);
                    let _ = std::fs::remove_file(&wav);
                    if ok {
                        return;
                    }
                }
                let mut spd = std::process::Command::new("spd-say");
                spd.args(["-l", espeak_voice, "--", &word]);
                if !play_tracked(spd) {
                    let mut espeak = std::process::Command::new("espeak-ng");
                    espeak.args(["-v", espeak_voice, "-s", espeak_wpm, "--", &word]);
                    let _ = play_tracked(espeak);
                }
                let _ = std::fs::remove_file(&wav);
                let _ = std::fs::remove_file(&mp3);
            });
        }
        #[cfg(all(target_os = "macos", not(test)))]
        {
            let say_wpm = speed.say_wpm();
            let _ = (lang, seq);
            std::thread::spawn(move || {
                if let Ok(mut child) = std::process::Command::new("say").args(["-r", say_wpm, &word]).spawn() {
                    preview_pid.store(child.id(), std::sync::atomic::Ordering::SeqCst);
                    let _ = child.wait();
                }
                preview_pid.store(0, std::sync::atomic::Ordering::SeqCst);
            });
        }
        #[cfg(any(test, target_arch = "wasm32", target_os = "android", target_os = "ios"))]
        {
            let _ = (&word, lang, speed, seq, preview_pid);
        }
    }

    pub(super) fn pause_or_resume_tts(&mut self) {
        use std::sync::atomic::Ordering;
        let pid = self.tts_pid.load(Ordering::SeqCst);
        match self.tts_state {
            TtsState::Playing => {
                self.tts_paused_flag.store(true, Ordering::SeqCst);
                if let Ok(mut sync) = self.tts_sync.lock()
                    && let Some(start) = sync.playback_started_at.take()
                {
                    sync.elapsed_before_pause += start.elapsed().as_secs_f32();
                }
                self.tts_state = TtsState::Paused;
                self.pending_speech_cmd = Some("pause");
                #[cfg(all(unix, not(test)))]
                if pid > 0 {
                    let _ = std::process::Command::new("kill").args(["-STOP", &pid.to_string()]).status();
                }
                #[cfg(not(all(unix, not(test))))]
                let _ = pid;
                self.say("Lecture en pause.");
            }
            TtsState::Paused => {
                self.tts_paused_flag.store(false, Ordering::SeqCst);
                if let Ok(mut sync) = self.tts_sync.lock()
                    && !sync.preparing
                {
                    sync.playback_started_at = Some(Instant::now());
                }
                self.tts_state = TtsState::Playing;
                self.pending_speech_cmd = Some("resume");
                #[cfg(all(unix, not(test)))]
                if pid > 0 {
                    let _ = std::process::Command::new("kill").args(["-CONT", &pid.to_string()]).status();
                }
                #[cfg(not(all(unix, not(test))))]
                let _ = pid;
                self.say("Reprise de la lecture.");
            }
            TtsState::Stopped => {}
        }
    }

    pub(super) fn stop_tts(&mut self) {
        use std::sync::atomic::Ordering;
        self.tts_stop_flag.store(true, Ordering::SeqCst);
        self.tts_paused_flag.store(false, Ordering::SeqCst);
        self.tts_state = TtsState::Stopped;
        if let Ok(mut sync) = self.tts_sync.lock() {
            sync.preparing = false;
            sync.playback_started_at = None;
            sync.elapsed_before_pause = 0.0;
            sync.direct_word_index = None;
        }
        self.pending_speech_cmd = Some("stop");
        let pid = self.tts_pid.swap(0, Ordering::SeqCst);
        #[cfg(all(unix, not(test)))]
        {
            if pid > 0 {
                let s = pid.to_string();
                let _ = std::process::Command::new("kill").args(["-CONT", &s]).status();
                let _ = std::process::Command::new("kill").args(["-TERM", &s]).status();
            }
            let _ = std::process::Command::new("pkill").args(["-f", "ergonote_tts\\."]).status();
        }
        #[cfg(all(target_os = "windows", not(test)))]
        if pid > 0 {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            let _ = std::process::Command::new("taskkill").args(["/PID", &pid.to_string(), "/T", "/F"]).creation_flags(CREATE_NO_WINDOW).status();
        }
        #[cfg(any(test, target_arch = "wasm32", target_os = "android"))]
        let _ = pid;
    }

    pub(super) fn tts_sync_snapshot(&self) -> (bool, Option<usize>, Vec<TtsWordCue>) {
        if self.tts_state == TtsState::Stopped {
            return (false, None, Vec::new());
        }
        let Ok(sync) = self.tts_sync.lock() else {
            return (false, None, Vec::new());
        };
        if sync.cues.is_empty() {
            return (sync.preparing, None, Vec::new());
        }
        if sync.preparing {
            return (true, Some(0), sync.cues.clone());
        }
        if let Some(direct) = sync.direct_word_index {
            return (false, Some(direct.min(sync.cues.len() - 1)), sync.cues.clone());
        }
        let elapsed = match self.tts_state {
            TtsState::Playing => sync.elapsed_before_pause + sync.playback_started_at.map_or(0.0, |s| s.elapsed().as_secs_f32()),
            TtsState::Paused => sync.elapsed_before_pause,
            TtsState::Stopped => 0.0,
        };
        let pos = sync.cues.iter().rposition(|cue| elapsed >= cue.start_sec).unwrap_or(0);
        (false, Some(pos), sync.cues.clone())
    }

    pub(super) fn tts_overlay(&mut self, ctx: &Context) {
        if self.tts_done.swap(false, std::sync::atomic::Ordering::SeqCst) {
            self.tts_state = TtsState::Stopped;
            if let Ok(mut sync) = self.tts_sync.lock() {
                sync.preparing = false;
                sync.playback_started_at = None;
                sync.direct_word_index = None;
            }
        }
        if self.tts_state == TtsState::Stopped {
            return;
        }
        if self.tts_state == TtsState::Playing {
            ctx.request_repaint_after(std::time::Duration::from_millis(50));
        }
        let t = self.tokens;
        let mut toggle_pause = false;
        let mut do_stop = false;
        let mut new_speed: Option<TtsSpeed> = None;
        let mut read_line = false;
        let (preparing, _, _) = self.tts_sync_snapshot();
        let has_caret = self.editor.as_ref().is_some_and(|e| e.caret.is_some());
        egui::Area::new(Id::new("tts-player-bar"))
            .order(egui::Order::Foreground)
            .anchor(Align2::CENTER_TOP, vec2(0.0, self.band_bottom + 10.0))
            .show(ctx, |ui| {
                egui::Frame::popup(ui.style())
                    .fill(t.panel)
                    .stroke(egui::Stroke::new(1.5, t.accent))
                    .corner_radius(10.0)
                    .inner_margin(egui::Margin::symmetric(12, 8))
                    .show(ui, |ui| {
                        ui.set_max_width((ctx.content_rect().width() - 32.0).min(740.0));
                        ui.horizontal_wrapped(|ui| {
                            ui.spacing_mut().item_spacing = vec2(6.0, 4.0);
                            let (icon_r, _) = ui.allocate_exact_size(vec2(16.0, 16.0), Sense::hover());
                            icons::paint(&ui.painter_at(icon_r), icon_r, Icon::ReadAloud, t.accent);
                            let status = if preparing {
                                "Préparation de la voix…"
                            } else if self.tts_state == TtsState::Playing {
                                "Lecture en cours"
                            } else {
                                "En pause"
                            };
                            ui.label(egui::RichText::new(status).strong().size(13.5).color(t.text));
                            let pause_label = if self.tts_state == TtsState::Playing { "Pause" } else { "Reprendre" };
                            let pause_btn = egui::Button::new(egui::RichText::new(pause_label).strong().color(t.strong))
                                .fill(crate::theme::mix(t.panel, t.accent, 0.18))
                                .corner_radius(6.0)
                                .min_size(vec2(74.0, 26.0));
                            if ui.add(pause_btn).clicked() {
                                toggle_pause = true;
                            }
                            let stop_btn = egui::Button::new(egui::RichText::new("Fin").strong().color(t.warn))
                                .fill(crate::theme::mix(t.panel, t.warn, 0.14))
                                .corner_radius(6.0)
                                .min_size(vec2(52.0, 26.0));
                            if ui.add(stop_btn).clicked() {
                                do_stop = true;
                            }
                            ui.separator();
                            ui.label(egui::RichText::new("Vitesse :").size(12.0).color(t.muted));
                            for speed in TtsSpeed::ALL {
                                let active = self.tts_speed == speed;
                                let btn = egui::Button::new(egui::RichText::new(speed.label()).size(12.0).strong().color(if active {
                                    Color32::WHITE
                                } else {
                                    t.text
                                }))
                                .fill(if active { t.accent } else { crate::theme::mix(t.panel, t.editor, 0.7) })
                                .corner_radius(5.0)
                                .min_size(vec2(46.0, 24.0));
                                if ui.add(btn).clicked() && self.tts_speed != speed {
                                    new_speed = Some(speed);
                                }
                            }
                            if has_caret {
                                ui.separator();
                                if ui
                                    .add(
                                        egui::Button::new(egui::RichText::new("Lire cette ligne").size(12.0).color(t.strong))
                                            .fill(crate::theme::mix(t.panel, t.accent, 0.12))
                                            .corner_radius(5.0)
                                            .min_size(vec2(0.0, 24.0)),
                                    )
                                    .clicked()
                                {
                                    read_line = true;
                                }
                            }
                        });
                    });
            });
        if do_stop {
            self.stop_tts();
            self.say("Lecture arrêtée.");
        } else if read_line {
            self.read_active_line_aloud();
        } else if let Some(speed) = new_speed {
            self.tts_speed = speed;
            let text = self.tts_last_text.clone();
            if !self.tts_words.is_empty() && !text.is_empty() {
                let parts: Vec<(Option<usize>, String, (usize, usize), (usize, usize))> =
                    self.tts_words.iter().map(|c| (c.block, c.word.clone(), c.caret_chars, c.drawn_bytes)).collect();
                let (clean, rebuilt) = Self::build_cues_from_spoken_parts(&parts, speed);
                self.speak_cues_with_speed(&clean, rebuilt);
            } else if !text.is_empty() {
                self.speak_text_with_speed(&text);
            }
        } else if toggle_pause {
            self.pause_or_resume_tts();
        }
    }

    pub(super) fn apply_word_suggestion(&mut self, char_range: (usize, usize), replacement: &str) {
        let Some(ed) = self.editor.as_mut() else { return };
        let Some((lo, hi, chars)) = ed.caret.as_ref().and_then(|caret| {
            let chars: Vec<char> = caret.text.chars().collect();
            let lo = char_range.0.min(chars.len());
            let hi = char_range.1.min(chars.len());
            (lo <= hi).then_some((lo, hi, chars))
        }) else {
            return;
        };
        Self::remember_step(ed);
        let after = &chars[hi..];
        let needs_space = after.first().is_none_or(|ch| !ch.is_whitespace());
        let mut updated: String = chars[..lo].iter().collect();
        updated.push_str(replacement);
        if needs_space {
            updated.push(' ');
        }
        let cursor_pos = lo + replacement.chars().count() + usize::from(needs_space);
        updated.extend(after.iter());

        let Some(caret) = ed.caret.as_mut() else { return };
        caret.text = updated;
        caret.selection = (cursor_pos, cursor_pos);
        caret.place = Some(cursor_pos);
        caret.focus = true;
        ed.restore_format_selection = Some((caret.block, cursor_pos, cursor_pos));
        if let Some(b) = ed.blocks.get_mut(caret.block) {
            caret.write(b);
        }
        ed.rebuild_body();
        Self::mark_dirty_from_blocks(ed);
        Self::remember_edit(ed);
        self.prediction_cache = None;
        self.prediction_selected = None;
    }

    #[cfg(all(not(test), not(target_arch = "wasm32"), not(any(target_os = "android", target_os = "ios"))))]
    pub(super) fn query_languagetool_online(text: &str, lang: crate::spellcheck::SpellLang) -> Option<String> {
        let lt_lang = match lang {
            crate::spellcheck::SpellLang::Fr => "fr",
            crate::spellcheck::SpellLang::En => "en-US",
            crate::spellcheck::SpellLang::De => "de-DE",
            crate::spellcheck::SpellLang::Es => "es",
            crate::spellcheck::SpellLang::It => "it",
        };
        let mut cmd = std::process::Command::new("curl");
        cmd.args([
            "-sS",
            "--max-time",
            "2",
            "--connect-timeout",
            "1",
            "-X",
            "POST",
            "https://api.languagetool.org/v2/check",
            "--data-urlencode",
            &format!("text={text}"),
            "--data",
            &format!("language={lt_lang}"),
        ]);
        #[cfg(target_os = "windows")]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            cmd.creation_flags(CREATE_NO_WINDOW);
        }
        let out = cmd.output().ok()?;
        if out.status.success() { String::from_utf8(out.stdout).ok() } else { None }
    }

    pub(super) fn apply_orthography_fix(&mut self, block_idx: usize, char_range: (usize, usize), replacement: &str) -> isize {
        let Some(ed) = self.editor.as_mut() else { return 0 };
        let Some(blk) = ed.blocks.get(block_idx) else { return 0 };
        let current_text = if let Some(c) = ed.caret.as_ref()
            && c.block == block_idx
        {
            c.text.clone()
        } else {
            pageedit::caret_text(blk)
        };
        let chars: Vec<char> = current_text.chars().collect();
        let lo = char_range.0.min(chars.len());
        let hi = char_range.1.min(chars.len());
        if lo >= hi {
            return 0;
        }
        Self::remember_step(ed);
        let mut updated: String = chars[..lo].iter().collect();
        updated.push_str(replacement);
        updated.extend(chars[hi..].iter());
        let rep_len = replacement.chars().count();
        let delta = isize::try_from(rep_len).unwrap_or(0) - isize::try_from(hi - lo).unwrap_or(0);
        let new_caret = lo + rep_len;

        if let Some(c) = ed.caret.as_mut()
            && c.block == block_idx
        {
            c.text = updated;
            c.selection = (new_caret, new_caret);
            c.place = Some(new_caret);
            c.focus = true;
            ed.restore_format_selection = Some((block_idx, new_caret, new_caret));
            if let Some(b) = ed.blocks.get_mut(block_idx) {
                c.write(b);
            }
        } else if let Some(b) = ed.blocks.get_mut(block_idx) {
            let mut tmp = pageedit::Caret {
                block: block_idx,
                text: updated,
                focus: false,
                selection: (0, 0),
                place: None,
                cell: None,
                span: None,
                inline: None,
            };
            tmp.write(b);
        }
        ed.rebuild_body();
        Self::mark_dirty_from_blocks(ed);
        Self::remember_edit(ed);
        self.prediction_cache = None;
        self.prediction_selected = None;
        delta
    }

    pub(super) fn orthography_corrected_preview(
        target_blocks: &[(usize, usize, String)],
        issues: &[(usize, crate::spellcheck::GrammarIssue)],
    ) -> String {
        let mut parts = Vec::with_capacity(target_blocks.len());
        for (b_idx, slice_start, txt) in target_blocks {
            let mut chars: Vec<char> = txt.chars().collect();
            let slice_end = slice_start + chars.len();
            let mut local_fixes: Vec<(usize, usize, &str)> = issues
                .iter()
                .filter_map(|(ib, iss)| {
                    if *ib != *b_idx || iss.char_range.0 < *slice_start || iss.char_range.1 > slice_end {
                        return None;
                    }
                    let first = iss.suggestions.first()?;
                    Some((iss.char_range.0 - slice_start, iss.char_range.1 - slice_start, first.as_str()))
                })
                .collect();
            local_fixes.sort_by_key(|f| f.0);
            for (lo, hi, rep) in local_fixes.into_iter().rev() {
                if lo <= hi && hi <= chars.len() {
                    chars.splice(lo..hi, rep.chars());
                }
            }
            parts.push(chars.into_iter().collect::<String>());
        }
        parts.join("\n")
    }

    pub(super) fn collect_orthography_targets(&self, ed: &Editor) -> Vec<(usize, usize, String)> {
        let mut targets: Vec<(usize, usize, String)> = Vec::new();
        if let Some(c) = ed.caret.as_ref()
            && c.selection.0 != c.selection.1
        {
            let lo = c.selection.0.min(c.selection.1);
            let hi = c.selection.0.max(c.selection.1);
            let chars: Vec<char> = c.text.chars().collect();
            let hi = hi.min(chars.len());
            if lo < hi {
                targets.push((c.block, lo, chars[lo..hi].iter().collect()));
                return targets;
            }
        }
        if let Some(open) = ed.across {
            for b_idx in open.blocks() {
                if let Some(blk) = ed.blocks.get(b_idx)
                    && matches!(
                        blk,
                        ergonote_formats::markdown::Block::Heading { .. }
                            | ergonote_formats::markdown::Block::Paragraph(_)
                            | ergonote_formats::markdown::Block::Item { .. }
                            | ergonote_formats::markdown::Block::Quote(_)
                    )
                {
                    let whole = pageedit::block_text(blk);
                    if let Some((ba, bb)) = open.covering(b_idx, whole.len()) {
                        let ca = whole.get(..ba).map_or(0, |s| s.chars().count());
                        let cb = whole.get(..bb).map_or_else(|| whole.chars().count(), |s| s.chars().count());
                        let caret_txt = pageedit::caret_text(blk);
                        let prefix_chars = whole.chars().count().saturating_sub(caret_txt.chars().count());
                        let lo = ca.saturating_sub(prefix_chars);
                        let hi = cb.saturating_sub(prefix_chars);
                        let chars: Vec<char> = caret_txt.chars().collect();
                        let hi = hi.min(chars.len());
                        if lo < hi {
                            targets.push((b_idx, lo, chars[lo..hi].iter().collect()));
                        }
                    }
                }
            }
            if !targets.is_empty() {
                return targets;
            }
        }

        // Sans sélection explicite : si le bloc actif du curseur contient du texte avec des fautes,
        // analyser ce paragraphe en priorité afin de rester concentré sur la phrase de l'élève.
        if let Some(c) = ed.caret.as_ref()
            && !c.text.trim().is_empty()
            && !crate::spellcheck::scan_orthography_and_syntax(&c.text, self.spell_lang).is_empty()
        {
            targets.push((c.block, 0, c.text.clone()));
            return targets;
        }

        for (b_idx, blk) in ed.blocks.iter().enumerate() {
            if matches!(
                blk,
                ergonote_formats::markdown::Block::Heading { .. }
                    | ergonote_formats::markdown::Block::Paragraph(_)
                    | ergonote_formats::markdown::Block::Item { .. }
                    | ergonote_formats::markdown::Block::Quote(_)
            ) {
                let txt = if let Some(c) = ed.caret.as_ref()
                    && c.block == b_idx
                    && c.cell.is_none()
                {
                    c.text.clone()
                } else {
                    pageedit::caret_text(blk)
                };
                if !txt.trim().is_empty() {
                    targets.push((b_idx, 0, txt));
                }
            }
        }
        targets
    }

    pub(super) fn run_orthography_check(&mut self) {
        let Some(ed) = self.editor.as_ref() else {
            self.say(crate::tl_id!("ui.open_a_page_first"));
            return;
        };
        let targets = self.collect_orthography_targets(ed);
        let analyzed_text: String = targets.iter().map(|(_, _, s)| s.as_str()).collect::<Vec<_>>().join("\n");

        let mut issues: Vec<(usize, crate::spellcheck::GrammarIssue)> = Vec::new();
        for (b_idx, slice_start, txt) in &targets {
            for mut issue in crate::spellcheck::scan_orthography_and_syntax(txt, self.spell_lang) {
                issue.char_range.0 += *slice_start;
                issue.char_range.1 += *slice_start;
                issues.push((*b_idx, issue));
            }
        }

        #[cfg_attr(any(test, target_arch = "wasm32", target_os = "android", target_os = "ios"), allow(unused_mut))]
        let mut online_checked = false;
        #[cfg(all(not(test), not(target_arch = "wasm32"), not(any(target_os = "android", target_os = "ios"))))]
        {
            let total_chars: usize = targets.iter().map(|(_, _, s)| s.chars().count()).sum();
            if (2..=4000).contains(&total_chars) {
                let mut joined = String::new();
                let mut spans: Vec<(usize, usize, usize, usize)> = Vec::with_capacity(targets.len());
                let mut cursor = 0usize;
                for (b_idx, slice_start, txt) in &targets {
                    if !joined.is_empty() {
                        joined.push_str("\n\n");
                        cursor += 2;
                    }
                    let len = txt.chars().count();
                    spans.push((*b_idx, *slice_start, cursor, cursor + len));
                    joined.push_str(txt);
                    cursor += len;
                }
                if let Some(json) = Self::query_languagetool_online(&joined, self.spell_lang) {
                    online_checked = true;
                    for lt in crate::spellcheck::parse_languagetool_matches(&json, &joined) {
                        if let Some(&(b_idx, slice_start, j0, _j1)) =
                            spans.iter().find(|&&(_, _, j0, j1)| lt.char_range.0 >= j0 && lt.char_range.1 <= j1)
                        {
                            let mapped = (slice_start + lt.char_range.0 - j0, slice_start + lt.char_range.1 - j0);
                            if !issues.iter().any(|(eb, ei)| *eb == b_idx && ei.char_range.0 < mapped.1 && mapped.0 < ei.char_range.1) {
                                issues.push((b_idx, crate::spellcheck::GrammarIssue { char_range: mapped, ..lt }));
                            }
                        }
                    }
                    issues.sort_by_key(|(b, i)| (*b, i.char_range.0));
                }
            }
        }

        self.dialog = Some(Dialog::OrthographyCheck { analyzed_text, target_blocks: targets, issues, online_checked });
    }

    pub(super) fn support_page_url() -> String {
        if let Ok(cwd) = std::env::current_dir() {
            let local = cwd.join("presentation/soutenir.html");
            if local.is_file() {
                return local.to_string_lossy().into_owned();
            }
        }
        "https://masterz42.github.io/ErgoNote/soutenir.html".to_owned()
    }

    pub(super) fn feedback_platform_label() -> &'static str {
        if cfg!(target_os = "android") {
            "Android"
        } else if cfg!(target_os = "ios") {
            "iOS / iPadOS"
        } else if cfg!(target_os = "windows") {
            "Windows"
        } else if cfg!(target_os = "macos") {
            "macOS"
        } else if cfg!(target_arch = "wasm32") {
            "Web"
        } else {
            "Linux"
        }
    }

    pub(super) fn feedback_page_url() -> String {
        format!("https://masterz42.github.io/ErgoNote/retours.html?type=bug&v={}&os={}", crate::backstage::VERSION, Self::feedback_platform_label())
    }

    pub(super) fn current_word_analysis(&mut self) -> Option<(usize, usize, crate::spellcheck::WordAnalysis)> {
        if !self.word_prediction_enabled
            || self.show_source
            || self.dialog.is_some()
            || self.backstage.is_some()
            || self.custom_tags.is_some()
            || self.pointer != ribbon::Pointer::Write
            || self.compact_list_fills()
            || !self.search.trim().is_empty()
        {
            self.prediction_selected = None;
            return None;
        }
        let (caret_block, text, sel_pos, page_ctx, ctx_hash) = {
            use std::hash::{Hash, Hasher};
            let Some(ed) = self.editor.as_ref() else {
                self.prediction_selected = None;
                return None;
            };
            let Some(c) = ed.caret.as_ref() else {
                self.prediction_selected = None;
                return None;
            };
            if c.selection.0 != c.selection.1 || c.cell.is_some() {
                self.prediction_selected = None;
                return None;
            }
            let ok = !matches!(
                ed.blocks.get(c.block),
                Some(ergonote_formats::markdown::Block::Code { .. } | ergonote_formats::markdown::Block::MathBlock(_))
            );
            if !ok {
                self.prediction_selected = None;
                return None;
            }
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            ed.title_buf.hash(&mut hasher);
            for (b_idx, blk) in ed.blocks.iter().enumerate() {
                if b_idx != c.block {
                    b_idx.hash(&mut hasher);
                    pageedit::block_text(blk).hash(&mut hasher);
                }
            }
            let ctx_hash = hasher.finish();
            let cache_hit = self.prediction_cache.as_ref().and_then(|(b, txt, pos, lang, h, _)| {
                (*b == c.block && *txt == c.text && *pos == c.selection.0 && *lang == self.spell_lang && *h == ctx_hash).then_some(())
            });
            let mut page_ctx = String::new();
            if cache_hit.is_none() {
                if !ed.title_buf.trim().is_empty() {
                    page_ctx.push_str(&ed.title_buf);
                    page_ctx.push('\n');
                }
                for (b_idx, blk) in ed.blocks.iter().enumerate() {
                    if b_idx != c.block {
                        let t = pageedit::block_text(blk);
                        if !t.trim().is_empty() {
                            page_ctx.push_str(&t);
                            page_ctx.push('\n');
                        }
                    }
                }
            }
            (c.block, c.text.clone(), c.selection.0, page_ctx, ctx_hash)
        };
        let cached = self.prediction_cache.as_ref().and_then(|(b, txt, pos, lang, h, res)| {
            (*b == caret_block && *txt == text && *pos == sel_pos && *lang == self.spell_lang && *h == ctx_hash).then(|| res.clone())
        });
        let analysis = match cached {
            Some(found) => found,
            None => {
                self.prediction_selected = None;
                let computed = crate::spellcheck::analyze_word_at_caret_with_context(&text, sel_pos, self.spell_lang, &page_ctx);
                self.prediction_cache = Some((caret_block, text, sel_pos, self.spell_lang, ctx_hash, computed.clone()));
                computed
            }
        }?;
        Some((caret_block, sel_pos, analysis))
    }

    /// Handles arrow navigation (Up/Down/Left/Right + spoken preview) and Enter validation in the
    /// word-prediction popup before `pageedit` consumes keys on the active `TextEdit`.
    pub(super) fn prediction_keys(&mut self, ctx: &Context) {
        let Some((_, caret_at, analysis)) = self.current_word_analysis() else {
            return;
        };
        let count = analysis.suggestions.len().min(crate::spellcheck::MAX_SUGGESTIONS);
        if count == 0 {
            self.prediction_selected = None;
            return;
        }
        let Some(cur) = self.prediction_selected else {
            let enter_via_down = ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::ArrowDown));
            let enter_via_right =
                !enter_via_down && caret_at >= analysis.char_range.1 && ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::ArrowRight));
            if enter_via_down || enter_via_right {
                self.prediction_selected = Some(0);
                if let Some(word) = analysis.suggestions.first() {
                    self.speak_word_preview(word);
                }
            }
            return;
        };
        let cur = cur.min(count - 1);
        if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape)) {
            self.prediction_selected = None;
            return;
        }
        if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Enter)) || ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Tab)) {
            let chosen = analysis.suggestions[cur].clone();
            self.prediction_selected = None;
            self.apply_word_suggestion(analysis.char_range, &chosen);
            self.speak_word_preview(&chosen);
            return;
        }
        if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::ArrowDown)) || ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::ArrowRight)) {
            let next = (cur + 1) % count;
            self.prediction_selected = Some(next);
            self.speak_word_preview(&analysis.suggestions[next]);
            return;
        }
        if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::ArrowUp)) || ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::ArrowLeft)) {
            let prev = if cur == 0 { count - 1 } else { cur - 1 };
            self.prediction_selected = Some(prev);
            self.speak_word_preview(&analysis.suggestions[prev]);
        }
    }

    pub(super) fn show_word_prediction_popup(&mut self, ctx: &Context) {
        if !self.word_prediction_enabled
            || self.show_source
            || self.dialog.is_some()
            || self.backstage.is_some()
            || self.custom_tags.is_some()
            || self.pointer != ribbon::Pointer::Write
            || self.compact_list_fills()
            || !self.search.trim().is_empty()
        {
            self.prediction_caret_pos = None;
            self.prediction_selected = None;
            return;
        }
        if let Some(pos) = ctx.data(|d| d.get_temp::<(usize, egui::Rect)>(pageedit::caret_screen_pos_id())) {
            self.prediction_caret_pos = Some(pos);
        }
        let Some((pos_block, caret_rect)) = self.prediction_caret_pos else {
            return;
        };
        let Some((caret_block, _, analysis)) = self.current_word_analysis() else {
            return;
        };
        if caret_block != pos_block || analysis.suggestions.is_empty() {
            return;
        }

        let t = self.tokens;
        let screen = ctx.content_rect();
        let popup_w = 242.0;
        let est_h = 54.0 + analysis.suggestions.len() as f32 * 28.0;
        let mut x = (caret_rect.min.x - 12.0).clamp(screen.min.x + 8.0, (screen.max.x - popup_w - 8.0).max(screen.min.x + 8.0));
        let y = if caret_rect.max.y + 6.0 + est_h <= screen.max.y - 28.0 {
            caret_rect.max.y + 6.0
        } else {
            (caret_rect.min.y - est_h - 6.0).max(self.band_bottom + 8.0)
        };
        if x.is_nan() || y.is_nan() {
            x = screen.min.x + 16.0;
        }

        let mut picked_word: Option<String> = None;
        let mut speak_word: Option<String> = None;
        let mut cycle_lang = false;

        egui::Area::new(Id::new("word-prediction-popup")).order(egui::Order::Foreground).fixed_pos(egui::pos2(x, y)).show(ctx, |ui| {
            egui::Frame::popup(ui.style())
                .fill(t.panel)
                .stroke(egui::Stroke::new(1.3, if analysis.is_misspelled { Color32::from_rgb(215, 65, 65) } else { t.accent }))
                .corner_radius(8.0)
                .inner_margin(egui::Margin::same(7))
                .show(ui, |ui| {
                    ui.set_width(popup_w);
                    ui.spacing_mut().item_spacing = vec2(4.0, 3.0);
                    ui.horizontal(|ui| {
                        let header = if analysis.is_misspelled {
                            "Correction"
                        } else if analysis.word.is_empty() {
                            "Mot suivant"
                        } else {
                            "Prédiction"
                        };
                        ui.label(egui::RichText::new(header).small().strong().color(if analysis.is_misspelled {
                            Color32::from_rgb(215, 65, 65)
                        } else {
                            t.accent
                        }));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            let lang_tip = format!("Langue active : {} (cliquer pour changer)", self.spell_lang.label());
                            if ui
                                .add(
                                    egui::Button::new(egui::RichText::new(self.spell_lang.badge()).small().strong().color(t.strong))
                                        .fill(crate::theme::mix(t.panel, t.accent, 0.16))
                                        .corner_radius(4.0)
                                        .min_size(vec2(30.0, 18.0)),
                                )
                                .on_hover_text(lang_tip)
                                .clicked()
                            {
                                cycle_lang = true;
                            }
                        });
                    });
                    ui.separator();
                    for (idx, sugg) in analysis.suggestions.iter().take(crate::spellcheck::MAX_SUGGESTIONS).enumerate() {
                        let is_selected = self.prediction_selected == Some(idx);
                        ui.horizontal(|ui| {
                            let label_txt = if is_selected { format!("> {sugg}") } else { sugg.clone() };
                            let txt_color = if is_selected { Color32::WHITE } else { t.strong };
                            let fill = if is_selected { t.accent } else { crate::theme::mix(t.panel, t.editor, 0.75) };
                            let mut word_btn = egui::Button::new(egui::RichText::new(label_txt).size(14.5).strong().color(txt_color))
                                .fill(fill)
                                .corner_radius(5.0)
                                .min_size(vec2(popup_w - 40.0, 24.0));
                            if is_selected {
                                word_btn = word_btn.stroke(egui::Stroke::new(1.5, Color32::WHITE));
                            }
                            if ui.add(word_btn).clicked() {
                                picked_word = Some(sugg.clone());
                            }
                            let (srect, sresp) = ui.allocate_exact_size(vec2(26.0, 24.0), Sense::click());
                            let sp = ui.painter_at(srect);
                            let sfill =
                                if sresp.hovered() { crate::theme::mix(t.panel, t.accent, 0.28) } else { crate::theme::mix(t.panel, t.accent, 0.14) };
                            sp.rect_filled(srect, 5.0, sfill);
                            icons::paint(&sp, srect.shrink(4.0), Icon::ReadAloud, t.accent);
                            if sresp.on_hover_text("Écouter ce mot").clicked() {
                                speak_word = Some(sugg.clone());
                            }
                        });
                    }
                    let hint = if self.prediction_selected.is_some() {
                        "Flèches : choisir | Entrée : valider"
                    } else {
                        "Flèche Bas/Droite : écouter & choisir"
                    };
                    ui.label(egui::RichText::new(hint).size(10.5).color(t.muted));
                });
        });

        if cycle_lang {
            self.spell_lang = self.spell_lang.next();
            self.prediction_cache = None;
            self.prediction_selected = None;
            if let Some(ed) = self.editor.as_mut()
                && let Some(c) = ed.caret.as_mut()
            {
                c.focus = true;
                ed.restore_format_selection = Some((c.block, c.selection.0, c.selection.1));
            }
            self.say(format!("Correcteur et prédiction : {}.", self.spell_lang.label()));
        } else if let Some(word) = speak_word {
            if let Some(ed) = self.editor.as_mut()
                && let Some(c) = ed.caret.as_mut()
            {
                c.focus = true;
                ed.restore_format_selection = Some((c.block, c.selection.0, c.selection.1));
            }
            self.speak_word_preview(&word);
        } else if let Some(word) = picked_word {
            self.apply_word_suggestion(analysis.char_range, &word);
            self.speak_word_preview(&word);
        }
    }

    pub(super) fn insert_dictated_text(&mut self, text: &str) {
        let clean = text.trim();
        if clean.is_empty() || self.editor.is_none() {
            return;
        }
        let needs_leading_space = self.editor.as_ref().and_then(|ed| ed.caret.as_ref()).is_some_and(|c| {
            let at = c.selection.0.min(c.selection.1);
            at > 0 && c.text.chars().nth(at - 1).is_some_and(|ch| !ch.is_whitespace())
        });
        let snippet = if needs_leading_space { format!(" {clean}") } else { clean.to_string() };
        self.insert_in_page(&snippet);
        self.save_in_background();
    }

    pub(super) fn toggle_voice_dictation(&mut self) {
        if self.editor.is_none() {
            self.say(crate::tl_id!("ui.open_a_page_first"));
            return;
        }
        if self.dictation_state == DictationState::Listening {
            self.finish_voice_dictation(false);
            return;
        }
        #[cfg(target_os = "android")]
        {
            self.dictation_state = DictationState::Listening;
            self.android_pick_request = Some(format!("voice/dictation:{}", self.spell_lang.tts_locale()));
            self.say("Dictée vocale Android lancée...");
            return;
        }
        #[cfg(target_os = "ios")]
        {
            self.say("La dictée vocale iOS n'est pas encore connectée (utilisez le micro du clavier iPadOS).");
            return;
        }
        #[cfg(not(any(target_os = "android", target_os = "ios")))]
        self.start_desktop_dictation();
    }

    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    pub(super) fn start_desktop_dictation(&mut self) {
        use std::sync::atomic::Ordering;
        self.dictation_stop_flag.store(false, Ordering::SeqCst);
        self.dictation_done.store(false, Ordering::SeqCst);
        if let Ok(mut guard) = self.dictation_result.lock() {
            *guard = None;
        }
        let locale = self.spell_lang.tts_locale().to_string();
        let lang_code = self.spell_lang.code().to_string();
        let pid_slot = self.dictation_pid.clone();
        let stop_flag = self.dictation_stop_flag.clone();
        let done_slot = self.dictation_done.clone();
        let out_slot = self.dictation_result.clone();

        #[cfg(all(target_os = "windows", not(test)))]
        {
            self.dictation_state = DictationState::Listening;
            let _ = (&lang_code, &stop_flag);
            std::thread::spawn(move || {
                use std::os::windows::process::CommandExt;
                const CREATE_NO_WINDOW: u32 = 0x0800_0000;
                let tmp = std::env::temp_dir().join("ergonote_dictation.txt");
                let _ = std::fs::remove_file(&tmp);
                let out_str = tmp.to_string_lossy().replace('\'', "''");
                let ps = format!(
                    "Add-Type -AssemblyName System.Speech; \
                     try {{ $r = New-Object System.Speech.Recognition.SpeechRecognitionEngine([System.Globalization.CultureInfo]::GetCultureInfo('{locale}')) }} catch {{ $r = New-Object System.Speech.Recognition.SpeechRecognitionEngine }}; \
                     $r.LoadGrammar((New-Object System.Speech.Recognition.DictationGrammar)); \
                     $r.SetInputToDefaultAudioDevice(); \
                     $res = $r.Recognize([TimeSpan]::FromSeconds(15)); \
                     if ($res -and $res.Text) {{ [System.IO.File]::WriteAllText('{out_str}', $res.Text, [System.Text.Encoding]::UTF8) }}"
                );
                let res = std::process::Command::new("powershell")
                    .args(["-NoProfile", "-NonInteractive", "-WindowStyle", "Hidden", "-Command", &ps])
                    .creation_flags(CREATE_NO_WINDOW)
                    .spawn();
                let outcome = match res {
                    Ok(mut child) => {
                        pid_slot.store(child.id(), Ordering::SeqCst);
                        let _ = child.wait();
                        pid_slot.store(0, Ordering::SeqCst);
                        match std::fs::read_to_string(&tmp) {
                            Ok(txt) if !txt.trim().is_empty() => Ok(txt.trim().to_string()),
                            _ => Err("Aucune phrase reconnue (vérifiez le micro Windows dans Paramètres > Reconnaissance vocale).".to_string()),
                        }
                    }
                    Err(e) => Err(format!("Impossible de démarrer la dictée Windows : {e}")),
                };
                if let Ok(mut guard) = out_slot.lock() {
                    *guard = Some(outcome);
                }
                done_slot.store(true, Ordering::SeqCst);
            });
            self.say("Parlez maintenant dans le micro (Windows)...");
        }

        #[cfg(all(target_os = "linux", not(test)))]
        {
            let wav = "/tmp/ergonote_dictation.wav";
            let _ = std::fs::remove_file(wav);
            let candidates: [(&str, &[&str]); 4] = [
                ("pw-record", &[wav]),
                ("parecord", &["--channels=1", "--rate=16000", "--format=s16le", "--file-format=wav", wav]),
                ("arecord", &["-f", "S16_LE", "-r", "16000", "-c", "1", "-t", "wav", wav]),
                ("ffmpeg", &["-y", "-loglevel", "error", "-f", "alsa", "-i", "default", "-ar", "16000", "-ac", "1", wav]),
            ];
            let mut spawned = None;
            for (cmd, args) in candidates {
                if let Ok(child) = std::process::Command::new(cmd).args(args).spawn() {
                    spawned = Some(child);
                    break;
                }
            }
            let Some(mut child) = spawned else {
                self.say("Aucun outil d'enregistrement micro trouvé (installez pulseaudio-utils, pipewire ou alsa-utils).");
                return;
            };
            self.dictation_state = DictationState::Listening;
            pid_slot.store(child.id(), Ordering::SeqCst);
            std::thread::spawn(move || {
                let _ = child.wait();
                pid_slot.store(0, Ordering::SeqCst);
                if stop_flag.load(Ordering::SeqCst) {
                    done_slot.store(true, Ordering::SeqCst);
                    return;
                }
                let outcome = transcribe_audio_file_cmd(wav, &locale, &lang_code);
                if let Ok(mut guard) = out_slot.lock() {
                    *guard = Some(outcome);
                }
                done_slot.store(true, Ordering::SeqCst);
            });
            self.say("Dictée vocale Linux en cours... Cliquez « Terminer et écrire » quand vous avez fini.");
        }

        #[cfg(all(target_os = "macos", not(test)))]
        {
            let wav = "/tmp/ergonote_dictation.wav";
            let _ = std::fs::remove_file(wav);
            let candidates: [(&str, &[&str]); 2] = [
                ("rec", &["-c", "1", "-r", "16000", wav]),
                ("ffmpeg", &["-y", "-loglevel", "error", "-f", "avfoundation", "-i", ":0", "-ar", "16000", "-ac", "1", wav]),
            ];
            let mut spawned = None;
            for (cmd, args) in candidates {
                if let Ok(child) = std::process::Command::new(cmd).args(args).spawn() {
                    spawned = Some(child);
                    break;
                }
            }
            if let Some(mut child) = spawned {
                self.dictation_state = DictationState::Listening;
                pid_slot.store(child.id(), Ordering::SeqCst);
                std::thread::spawn(move || {
                    let _ = child.wait();
                    pid_slot.store(0, Ordering::SeqCst);
                    if stop_flag.load(Ordering::SeqCst) {
                        done_slot.store(true, Ordering::SeqCst);
                        return;
                    }
                    let outcome = transcribe_audio_file_cmd(wav, &locale, &lang_code);
                    if let Ok(mut guard) = out_slot.lock() {
                        *guard = Some(outcome);
                    }
                    done_slot.store(true, Ordering::SeqCst);
                });
                self.say("Dictée vocale macOS en cours... Cliquez « Terminer et écrire » quand vous avez fini.");
            } else {
                // Fallback to macOS system Dictation dialog trigger via AppleScript.
                let _ = std::process::Command::new("osascript")
                    .args(["-e", "tell application \"System Events\" to click menu item 1 of (every menu item of menu \"Édition\" of menu bar 1 whose name contains \"Dict\")"])
                    .spawn();
                self.say("Dictée système macOS demandée (ou appuyez deux fois sur Fn).");
            }
        }

        #[cfg(any(test, target_arch = "wasm32"))]
        {
            let _ = (locale, lang_code, pid_slot, stop_flag, done_slot, out_slot);
            self.dictation_state = DictationState::Listening;
        }
    }

    pub(super) fn finish_voice_dictation(&mut self, cancel: bool) {
        use std::sync::atomic::Ordering;
        if cancel {
            self.dictation_stop_flag.store(true, Ordering::SeqCst);
            self.dictation_state = DictationState::Idle;
        }
        let pid = self.dictation_pid.load(Ordering::SeqCst);
        #[cfg(all(unix, not(test)))]
        if pid > 0 {
            // SIGINT asks `arecord` / `parecord` / `pw-record` / `ffmpeg` to flush the WAV header cleanly.
            let _ = std::process::Command::new("kill").args(["-INT", &pid.to_string()]).status();
        }
        #[cfg(all(target_os = "windows", not(test)))]
        if pid > 0 && cancel {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            let _ = std::process::Command::new("taskkill").args(["/PID", &pid.to_string(), "/T", "/F"]).creation_flags(CREATE_NO_WINDOW).status();
        }
        #[cfg(any(test, target_arch = "wasm32", target_os = "android"))]
        let _ = pid;
        if cancel {
            self.say("Dictée vocale annulée.");
        }
    }

    pub(super) fn poll_dictation(&mut self) {
        if !self.dictation_done.swap(false, std::sync::atomic::Ordering::SeqCst) {
            return;
        }
        self.dictation_state = DictationState::Idle;
        let outcome = self.dictation_result.lock().ok().and_then(|mut g| g.take());
        match outcome {
            Some(Ok(text)) if !text.trim().is_empty() => {
                let clean = text.trim().to_string();
                self.insert_dictated_text(&clean);
                self.say(format!("Dictée insérée : « {} »", clean));
            }
            Some(Ok(_)) => self.say("Aucune parole détectée."),
            Some(Err(msg)) => self.say(msg),
            None => {}
        }
    }

    pub(super) fn dictation_overlay(&mut self, ctx: &Context) {
        if self.dictation_state != DictationState::Listening {
            return;
        }
        ctx.request_repaint_after(std::time::Duration::from_millis(120));
        let t = self.tokens;
        let mut finish = false;
        let mut cancel = false;
        egui::Area::new(Id::new("dictation-listening-bar"))
            .order(egui::Order::Foreground)
            .anchor(Align2::CENTER_TOP, vec2(0.0, self.band_bottom + 14.0))
            .show(ctx, |ui| {
                egui::Frame::popup(ui.style())
                    .fill(t.panel)
                    .stroke(egui::Stroke::new(1.5, Color32::from_rgb(215, 58, 58)))
                    .corner_radius(10.0)
                    .inner_margin(egui::Margin::symmetric(12, 8))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing = vec2(8.0, 0.0);
                            let (mic_r, _) = ui.allocate_exact_size(vec2(16.0, 16.0), Sense::hover());
                            icons::paint(&ui.painter_at(mic_r), mic_r, Icon::Mic, Color32::from_rgb(215, 58, 58));
                            ui.label(
                                egui::RichText::new(format!("Écoute vocale ({}) en cours...", self.spell_lang.label()))
                                    .strong()
                                    .size(13.5)
                                    .color(Color32::from_rgb(215, 58, 58)),
                            );
                            if !cfg!(target_os = "android")
                                && ui
                                    .add(
                                        egui::Button::new(egui::RichText::new("Terminer et écrire").strong().color(Color32::WHITE))
                                            .fill(t.accent)
                                            .corner_radius(6.0)
                                            .min_size(vec2(126.0, 26.0)),
                                    )
                                    .clicked()
                            {
                                finish = true;
                            }
                            if ui
                                .add(
                                    egui::Button::new(egui::RichText::new("Annuler").color(t.warn))
                                        .fill(crate::theme::mix(t.panel, t.warn, 0.14))
                                        .corner_radius(6.0)
                                        .min_size(vec2(70.0, 26.0)),
                                )
                                .clicked()
                            {
                                cancel = true;
                            }
                        });
                    });
            });
        if cancel {
            self.finish_voice_dictation(true);
        } else if finish {
            self.finish_voice_dictation(false);
        }
    }
}
