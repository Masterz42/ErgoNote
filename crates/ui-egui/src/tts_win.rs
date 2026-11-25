//! Windows neural speech synthesis helpers: online Edge Neural voices with WordBoundary sync
//! first, auto-cached offline Piper voice second, and legacy SAPI fallback handled by caller.

#[cfg(target_os = "windows")]
use std::path::{Path, PathBuf};
#[cfg(target_os = "windows")]
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

const TRUSTED_CLIENT_TOKEN: &str = "6A5AA1D4EAFF4E9FB37E23D68491D6F4";
pub const EDGE_SEC_MS_GEC_VERSION: &str = "2-143.0.3650.75";
#[cfg(target_os = "windows")]
const PIPER_WINDOWS_ZIP_URL: &str = "https://github.com/rhasspy/piper/releases/download/2023.11.14-2/piper_windows_amd64.zip";

const WIN_EPOCH_SECS: u64 = 11_644_473_600;
const CS64_FACTOR: u32 = 0x8289_3933;
const CS64_ADJ0: u32 = 0xf2f9_0b1a;
const CS64_ADJ1: u32 = 0x0b1a_0000;
const CS64_COEFFS: [(u32, u32); 14] = [
    (0xe82e_91b1, 0xb327_00d0),
    (0x44de_24c6, 0x88df_f51d),
    (0x3beb_65bc, 0x2951_4038),
    (0xc89c_5933, 0xb328_cbdf),
    (0x8d1e_c211, 0xd405_b4d5),
    (0x314e_f5d5, 0x26e5_5f3c),
    (0xe924_91b1, 0x6078_01c6),
    (0xf0a5_b9d5, 0x82b2_c1fb),
    (0x0606_4cf3, 0xfa51_a3da),
    (0xda79_fa60, 0x207c_c464),
    (0xbed2_0560, 0x3c3c_74f3),
    (0x89f9_c9f8, 0xd2d1_4b61),
    (0x35fa_38c6, 0xbb04_a2c1),
    (0xc562_6167 ^ 0xce0b_82a1, 0xc562_6167),
];

// Explicit constant check keeps exact original coefficient table uncluttered.
const CS64_LAST_COEFF: (u32, u32) = (0xcb49_e3c6, 0xc562_6167);

fn words_le4_even(data: &[u8]) -> Vec<u32> {
    let mut padded = Vec::with_capacity(data.len() + 7);
    padded.extend_from_slice(data);
    while !padded.len().is_multiple_of(4) {
        padded.push(0);
    }
    let mut words: Vec<u32> = padded.as_chunks::<4>().0.iter().map(|&c| u32::from_le_bytes(c)).collect();
    if !words.len().is_multiple_of(2) {
        words.push(0);
    }
    words
}

fn cs64_digest(data: &[u8], digest: &[u8; 32]) -> [u32; 2] {
    let key_a = u32::from_le_bytes([digest[0], digest[1], digest[2], digest[3]]) | 1;
    let key_b = u32::from_le_bytes([digest[4], digest[5], digest[6], digest[7]]) | 1;
    let factors = [key_a.wrapping_mul(CS64_FACTOR), key_b.wrapping_mul(CS64_FACTOR)];

    let mut terms = Vec::with_capacity(CS64_COEFFS.len() + 24);
    let mut offsets = Vec::with_capacity(CS64_COEFFS.len() + 24);
    for (idx, pair) in CS64_COEFFS.iter().enumerate() {
        let (coeff, off) = if idx == CS64_COEFFS.len() - 1 { CS64_LAST_COEFF } else { *pair };
        terms.push(coeff.wrapping_mul(factors[idx % 2]));
        offsets.push(off);
    }

    let mut prefixed = Vec::with_capacity(1 + data.len());
    prefixed.push(0);
    prefixed.extend_from_slice(data);
    for (idx, word) in words_le4_even(&prefixed).into_iter().enumerate() {
        let mut term = word.wrapping_mul(if idx.is_multiple_of(2) { key_a } else { key_b });
        let mut offset = word.rotate_left(16).wrapping_mul(if idx.is_multiple_of(2) { 0xef05_69fb } else { 0xc317_13db });
        if idx == 0 {
            term = term.wrapping_add(CS64_ADJ0.wrapping_mul(factors[0]));
            offset = offset.wrapping_add(CS64_ADJ1);
        }
        terms.push(term);
        offsets.push(offset);
    }

    let (mut swap_acc, mut swap_last, mut state, mut acc) = (0u32, 0u32, 0u32, 0u32);
    for (&[tx, ty], &[ox, oy]) in terms.as_chunks::<2>().0.iter().zip(offsets.as_chunks::<2>().0.iter()) {
        let t0 = ox.wrapping_add(tx).rotate_left(16);
        let q0 = t0.wrapping_mul(0x76f5_0f34);
        swap_acc = swap_acc.wrapping_add(q0.rotate_left(16).wrapping_mul(0xc3ef_ea97).wrapping_add(q0));

        let t1 = oy.wrapping_add(ty).rotate_left(16);
        let q1 = t1.wrapping_mul(0x1881_ce3c);
        swap_last = q1.rotate_left(16).wrapping_mul(0x35bd_1ec9).wrapping_add(q1);
        swap_acc = swap_acc.wrapping_add(swap_last);

        state = state.wrapping_mul(key_a).wrapping_add(tx);
        state = state.rotate_left(16).wrapping_mul(0xcf98_b111);
        state = state.rotate_left(16).wrapping_mul(0x8708_5b9f);
        state = state.rotate_left(16).wrapping_mul(0x12ce_b96d);
        state = state.rotate_left(16).wrapping_mul(0x257e_1d83);
        acc = acc.wrapping_add(state);

        let mut tt = state.wrapping_mul(key_b).wrapping_add(ty);
        tt = tt.rotate_left(16).wrapping_mul(0xa274_16f5);
        tt = tt.rotate_left(16).wrapping_mul(0xd383_96ff);
        state = tt.rotate_left(16).wrapping_mul(0x7c93_2b89).rotate_left(16).wrapping_mul(0xbfa4_9f69);
        acc = acc.wrapping_add(state);
    }
    [swap_last ^ state, swap_acc ^ acc]
}

const SHA256_INITIAL: [u32; 8] = [0x6a09_e667, 0xbb67_ae85, 0x3c6e_f372, 0xa54f_f53a, 0x510e_527f, 0x9b05_688c, 0x1f83_d9ab, 0x5be0_cd19];
const SHA256_SALT_STATE: [u32; 8] = [0xfd9f_bea2, 0x9124_3ce7, 0xbe49_9094, 0xcd2e_1e0a, 0x6c00_6f49, 0x04bd_35ab, 0xfeb0_a7e4, 0x03ef_fbd2];
const SHA256_SALT_SCHEDULE: [u32; 14] = [
    0xeb24_bf9e,
    0x1558_691f,
    0xa0c0_9ee2,
    0x0cfa_32ba,
    0xa622_ee8c,
    0xd1dd_9491,
    0xc0dd_25a0,
    0xf9e7_0ab6,
    0xf0c3_1573,
    0x7058_ce4e,
    0x7188_e606,
    0x00e5_ab33,
    0xd258_b060,
    0x7276_6564,
];
const SHA256_K: [u32; 64] = [
    0x428a_2f98,
    0x7137_4491,
    0xb5c0_fbcf,
    0xe9b5_dba5,
    0x3956_c25b,
    0x59f1_11f1,
    0x923f_82a4,
    0xab1c_5ed5,
    0xd807_aa98,
    0x1283_5b01,
    0x2431_85be,
    0x550c_7dc3,
    0x72be_5d74,
    0x80de_b1fe,
    0x9bdc_06a7,
    0xc19b_f174,
    0xe49b_69c1,
    0xefbe_4786,
    0x0fc1_9dc6,
    0x240c_a1cc,
    0x2de9_2c6f,
    0x4a74_84aa,
    0x5cb0_a9dc,
    0x76f9_88da,
    0x983e_5152,
    0xa831_c66d,
    0xb003_27c8,
    0xbf59_7fc7,
    0xc6e0_0bf3,
    0xd5a7_9147,
    0x06ca_6351,
    0x1429_2967,
    0x27b7_0a85,
    0x2e1b_2138,
    0x4d2c_6dfc,
    0x5338_0d13,
    0x650a_7354,
    0x766a_0abb,
    0x81c2_c92e,
    0x9272_2c85,
    0xa2bf_e8a1,
    0xa81a_664b,
    0xc24b_8b70,
    0xc76c_51a3,
    0xd192_e819,
    0xd699_0624,
    0xf40e_3585,
    0x106a_a070,
    0x19a4_c116,
    0x1e37_6c08,
    0x2748_774c,
    0x34b0_bcb5,
    0x391c_0cb3,
    0x4ed8_aa4a,
    0x5b9c_ca4f,
    0x682e_6ff3,
    0x748f_82ee,
    0x78a5_636f,
    0x84c8_7814,
    0x8cc7_0208,
    0x90be_fffa,
    0xa450_6ceb,
    0xbef9_a3f7,
    0xc671_78f2,
];

fn expand_schedule(words: &mut [u32; 64], from: usize) {
    for i in from..64 {
        let x = words[i - 2];
        let y = words[i - 15];
        let s1 = x.rotate_right(17) ^ x.rotate_right(19) ^ (x >> 10);
        let s0 = y.rotate_right(7) ^ y.rotate_right(18) ^ (y >> 3);
        words[i] = s1.wrapping_add(words[i - 7]).wrapping_add(s0).wrapping_add(words[i - 16]);
    }
}

fn sha256_rounds(init: [u32; 8], words: &[u32; 64], from: usize, block14: bool) -> [u32; 8] {
    let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut h] = init;
    for i in from..64 {
        let k = if block14 && i == 14 { 0xc9dc_06a7 } else { SHA256_K[i] };
        let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
        let ch = (e & f) ^ ((!e) & g);
        let t1 = h.wrapping_add(s1).wrapping_add(ch).wrapping_add(k).wrapping_add(words[i]);
        let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
        let maj = (a & b) ^ (a & c) ^ (b & c);
        let t2 = s0.wrapping_add(maj);
        h = g;
        g = f;
        f = e;
        e = d.wrapping_add(t1);
        d = c;
        c = b;
        b = a;
        a = t1.wrapping_add(t2);
    }
    [a, b, c, d, e, f, g, h]
}

fn edge_sha256_digest(data: &[u8]) -> [u8; 32] {
    let mut words = [0u32; 64];
    if data.len() >= 7 {
        words[14] = u32::from_be_bytes([0, data[0], data[1], data[2]]);
        words[15] = u32::from_be_bytes([data[3], data[4], data[5], data[6]]);
    }
    for i in 16..30 {
        let mut v = SHA256_SALT_SCHEDULE[i - 16];
        if i >= 16 {
            let x = words[i - 2];
            let s1 = x.rotate_right(17) ^ x.rotate_right(19) ^ (x >> 10);
            v = v.wrapping_add(if i == 16 { s1 ^ 0x000b_92c0 } else { s1 });
        }
        if i >= 21 {
            v = v.wrapping_add(words[i - 7]);
        }
        if i >= 29 {
            let y = words[i - 15];
            let s0 = y.rotate_right(7) ^ y.rotate_right(18) ^ (y >> 3);
            v = v.wrapping_add(s0 ^ 0x059c_0b80);
        }
        words[i] = v;
    }
    let s1_28 = words[28].rotate_right(17) ^ words[28].rotate_right(19) ^ (words[28] >> 10);
    let s0_15 = words[15].rotate_right(7) ^ words[15].rotate_right(18) ^ (words[15] >> 3);
    words[30] = s1_28.wrapping_add(words[23]).wrapping_add(s0_15).wrapping_add(words[14]).wrapping_add(0xc9dc_06a7).wrapping_sub(SHA256_K[14]);
    expand_schedule(&mut words, 31);
    let working0 = sha256_rounds(SHA256_SALT_STATE, &words, 14, true);
    let mut state = [0u32; 8];
    for idx in 0..8 {
        state[idx] = SHA256_INITIAL[idx].wrapping_add(working0[idx]);
    }

    let length = 57 + data.len();
    let tail = data.get(7..).unwrap_or(&[]);
    let mut padded = Vec::with_capacity(tail.len() + 72);
    padded.extend_from_slice(tail);
    padded.push(0x80);
    let zeros = (55usize.wrapping_sub(length)) % 64;
    padded.extend(std::iter::repeat_n(0u8, zeros));
    padded.extend_from_slice(&((length as u64) * 8).to_be_bytes());
    for block in padded.as_chunks::<64>().0 {
        let mut bw = [0u32; 64];
        for (dst, ch) in bw.iter_mut().zip(block.as_chunks::<4>().0.iter()) {
            *dst = u32::from_be_bytes(*ch);
        }
        expand_schedule(&mut bw, 16);
        let working = sha256_rounds(state, &bw, 0, false);
        for idx in 0..8 {
            state[idx] = state[idx].wrapping_add(working[idx]);
        }
    }
    let mut out = [0u8; 32];
    for (dst, word) in out.as_chunks_mut::<4>().0.iter_mut().zip(state.iter()) {
        *dst = word.to_be_bytes();
    }
    out
}

/// Computes the v2 `Sec-MS-GEC` anti-abuse token required by Microsoft Edge TTS endpoints.
pub fn edge_sec_ms_gec(unix_secs: u64) -> String {
    let aligned = unix_secs + WIN_EPOCH_SECS;
    let rounded = aligned - (aligned % 300);
    let ticks = u128::from(rounded) * 10_000_000;
    let payload = format!("{ticks}{TRUSTED_CLIENT_TOKEN}");
    let digest = edge_sha256_digest(payload.as_bytes());
    let cs = cs64_digest(payload.as_bytes(), &digest);
    let mut out = String::with_capacity(81);
    for word in cs {
        for b in word.to_le_bytes() {
            out.push_str(&format!("{b:02X}"));
        }
    }
    out.push('-');
    for b in digest {
        out.push_str(&format!("{b:02X}"));
    }
    out
}

/// Builds the authenticated Edge ReadAloud WebSocket URL.
pub fn edge_wss_url(unix_secs: u64, conn_id: &str) -> String {
    let gec = edge_sec_ms_gec(unix_secs);
    format!(
        "wss://speech.platform.bing.com/consumer/speech/synthesize/readaloud/edge/v1?TrustedClientToken={TRUSTED_CLIENT_TOKEN}&ConnectionId={conn_id}&Sec-MS-GEC={gec}&Sec-MS-GEC-Version={EDGE_SEC_MS_GEC_VERSION}"
    )
}

/// Formats XML-escaped SSML payload accepted by Edge ReadAloud.
pub fn edge_ssml(voice: &str, rate: &str, text: &str) -> String {
    let mut escaped = String::with_capacity(text.len() + 16);
    for ch in text.chars() {
        match ch {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&apos;"),
            _ => escaped.push(ch),
        }
    }
    format!(
        "<speak version='1.0' xmlns='http://www.w3.org/2001/10/synthesis' xml:lang='en-US'><voice name='{voice}'><prosody pitch='+0Hz' rate='{rate}' volume='+0%'>{escaped}</prosody></voice></speak>"
    )
}

/// Hugging Face v1.0.0 URLs `(onnx, onnx.json)` for each supported Piper offline neural voice.
pub fn piper_voice_urls(model_name: &str) -> Option<(&'static str, &'static str)> {
    match model_name {
        "fr_FR-siwis-medium.onnx" => Some((
            "https://huggingface.co/rhasspy/piper-voices/resolve/v1.0.0/fr/fr_FR/siwis/medium/fr_FR-siwis-medium.onnx",
            "https://huggingface.co/rhasspy/piper-voices/resolve/v1.0.0/fr/fr_FR/siwis/medium/fr_FR-siwis-medium.onnx.json",
        )),
        "en_US-lessac-medium.onnx" => Some((
            "https://huggingface.co/rhasspy/piper-voices/resolve/v1.0.0/en/en_US/lessac/medium/en_US-lessac-medium.onnx",
            "https://huggingface.co/rhasspy/piper-voices/resolve/v1.0.0/en/en_US/lessac/medium/en_US-lessac-medium.onnx.json",
        )),
        "de_DE-thorsten-medium.onnx" => Some((
            "https://huggingface.co/rhasspy/piper-voices/resolve/v1.0.0/de/de_DE/thorsten/medium/de_DE-thorsten-medium.onnx",
            "https://huggingface.co/rhasspy/piper-voices/resolve/v1.0.0/de/de_DE/thorsten/medium/de_DE-thorsten-medium.onnx.json",
        )),
        "es_ES-davefx-medium.onnx" => Some((
            "https://huggingface.co/rhasspy/piper-voices/resolve/v1.0.0/es/es_ES/davefx/medium/es_ES-davefx-medium.onnx",
            "https://huggingface.co/rhasspy/piper-voices/resolve/v1.0.0/es/es_ES/davefx/medium/es_ES-davefx-medium.onnx.json",
        )),
        "it_IT-paola-medium.onnx" => Some((
            "https://huggingface.co/rhasspy/piper-voices/resolve/v1.0.0/it/it_IT/paola/medium/it_IT-paola-medium.onnx",
            "https://huggingface.co/rhasspy/piper-voices/resolve/v1.0.0/it/it_IT/paola/medium/it_IT-paola-medium.onnx.json",
        )),
        _ => None,
    }
}

#[cfg(target_os = "windows")]
static PIPER_DOWNLOADING: AtomicBool = AtomicBool::new(false);

/// Local cache folder on Windows for the offline Piper neural binary and voices.
#[cfg(target_os = "windows")]
pub fn piper_dir() -> PathBuf {
    std::env::var_os("LOCALAPPDATA").map(PathBuf::from).unwrap_or_else(std::env::temp_dir).join("ErgoNote").join("tts")
}

#[cfg(target_os = "windows")]
fn download_file(url: &str, dest: &Path) -> bool {
    let Some(parent) = dest.parent() else {
        return false;
    };
    if std::fs::create_dir_all(parent).is_err() {
        return false;
    }
    let mut part_os = dest.as_os_str().to_os_string();
    part_os.push(".part");
    let part = PathBuf::from(part_os);
    let config = ureq::Agent::config_builder().timeout_global(Some(std::time::Duration::from_secs(180))).build();
    let agent: ureq::Agent = config.into();
    let Ok(resp) = agent.get(url).header("User-Agent", "ErgoNote-TTS/1.0").call() else {
        return false;
    };
    let mut reader = resp.into_body().into_reader();
    let Ok(mut out) = std::fs::File::create(&part) else {
        return false;
    };
    if std::io::copy(&mut reader, &mut out).is_err() {
        let _ = std::fs::remove_file(&part);
        return false;
    }
    if std::fs::rename(&part, dest).is_err() {
        let _ = std::fs::remove_file(&part);
        return false;
    }
    true
}

/// Quietly downloads the Windows Piper binary and requested voice model once in the background.
#[cfg(target_os = "windows")]
pub fn ensure_piper_in_background(model_name: &'static str) {
    let Some((onnx_url, json_url)) = piper_voice_urls(model_name) else {
        return;
    };
    let dir = piper_dir();
    let exe = dir.join("piper").join("piper.exe");
    let model = dir.join(model_name);
    let cfg = dir.join(format!("{model_name}.json"));
    if exe.is_file() && model.is_file() && cfg.is_file() {
        return;
    }
    if PIPER_DOWNLOADING.swap(true, Ordering::SeqCst) {
        return;
    }
    std::thread::spawn(move || {
        if !exe.is_file() {
            let zip_path = dir.join("piper_windows_amd64.zip");
            if download_file(PIPER_WINDOWS_ZIP_URL, &zip_path)
                && let Ok(file) = std::fs::File::open(&zip_path)
                && let Ok(mut archive) = zip::ZipArchive::new(file)
            {
                for idx in 0..archive.len() {
                    let Ok(mut entry) = archive.by_index(idx) else {
                        continue;
                    };
                    let Some(rel) = entry.enclosed_name() else {
                        continue;
                    };
                    let target = dir.join(rel);
                    if entry.is_dir() {
                        let _ = std::fs::create_dir_all(&target);
                    } else {
                        if let Some(parent) = target.parent() {
                            let _ = std::fs::create_dir_all(parent);
                        }
                        if let Ok(mut out) = std::fs::File::create(&target) {
                            let _ = std::io::copy(&mut entry, &mut out);
                        }
                    }
                }
            }
            let _ = std::fs::remove_file(&zip_path);
        }
        if !cfg.is_file() {
            let _ = download_file(json_url, &cfg);
        }
        if !model.is_file() {
            let _ = download_file(onnx_url, &model);
        }
        PIPER_DOWNLOADING.store(false, Ordering::SeqCst);
    });
}

/// Synthesizes MP3 audio and WordBoundary timestamps via Edge Neural WebSocket using PowerShell/.NET.
#[cfg(target_os = "windows")]
pub fn synthesize_edge_mp3(voice: &str, rate: &str, text: &str, out_paths: (&Path, &Path), pid_slot: &AtomicU32, stop_flag: &AtomicBool) -> bool {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    if stop_flag.load(Ordering::SeqCst) {
        return false;
    }
    let (mp3, tsv) = out_paths;
    let _ = std::fs::remove_file(mp3);
    let _ = std::fs::remove_file(tsv);
    let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.subsec_nanos()).unwrap_or(0);
    let conn_id = format!("{secs:016x}{nanos:08x}00000001");
    let url = edge_wss_url(secs, &conn_id);
    let ssml = edge_ssml(voice, rate, text);

    let ps_script = r#"
$ErrorActionPreference = 'Stop'
$ws = New-Object System.Net.WebSockets.ClientWebSocket
$ws.Options.SetRequestHeader('Origin', 'chrome-extension://jdiccldimpdaibmpdkjnbmckianbfold')
$ws.Options.SetRequestHeader('User-Agent', 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/143.0.0.0 Safari/537.36 Edg/143.0.0.0')
$ws.Options.SetRequestHeader('Pragma', 'no-cache')
$ws.Options.SetRequestHeader('Cache-Control', 'no-cache')
$cts = New-Object System.Threading.CancellationTokenSource(18000)
$ws.ConnectAsync([Uri]$env:NOTECRAFT_EDGE_URL, $cts.Token).GetAwaiter().GetResult()
function Send-Str([string]$s) {
    $b = [Text.Encoding]::UTF8.GetBytes($s)
    $seg = New-Object System.ArraySegment[byte]($b, 0, $b.Length)
    $ws.SendAsync($seg, [System.Net.WebSockets.WebSocketMessageType]::Text, $true, $cts.Token).GetAwaiter().GetResult()
}
$ts = [DateTime]::UtcNow.ToString('ddd MMM dd yyyy HH:mm:ss') + ' GMT+0000 (Coordinated Universal Time)'
Send-Str ("X-Timestamp:$ts`r`nContent-Type:application/json; charset=utf-8`r`nPath:speech.config`r`n`r`n{`"context`":{`"synthesis`":{`"audio`":{`"metadataoptions`":{`"sentenceBoundaryEnabled`":`"false`",`"wordBoundaryEnabled`":`"true`"},`"outputFormat`":`"audio-24khz-48kbitrate-mono-mp3`"}}}}")
Send-Str ("X-RequestId:$($env:NOTECRAFT_EDGE_REQ)`r`nContent-Type:application/ssml+xml`r`nX-Timestamp:$ts`r`nPath:ssml`r`n`r`n$($env:NOTECRAFT_EDGE_SSML)")
$mp3 = [IO.File]::Create($env:NOTECRAFT_TTS_MP3)
$tsv = New-Object IO.StreamWriter($env:NOTECRAFT_TTS_TSV, $false, [Text.UTF8Encoding]::new($false))
try {
    $buf = New-Object byte[] 65536
    $done = $false
    while (-not $done -and $ws.State -eq 'Open') {
        $ms = New-Object IO.MemoryStream
        do {
            $seg = New-Object System.ArraySegment[byte]($buf, 0, $buf.Length)
            $res = $ws.ReceiveAsync($seg, $cts.Token).GetAwaiter().GetResult()
            if ($res.MessageType -eq 'Close') { $done = $true; break }
            if ($res.Count -gt 0) { $ms.Write($buf, 0, $res.Count) }
        } while (-not $res.EndOfMessage)
        if ($done) { break }
        $bytes = $ms.ToArray()
        if ($res.MessageType -eq 'Binary' -and $bytes.Length -ge 2) {
            $hlen = ([int]$bytes[0] -shl 8) -bor [int]$bytes[1]
            $hdr = [Text.Encoding]::UTF8.GetString($bytes, 2, [Math]::Min($hlen, $bytes.Length - 2))
            if ($hdr.Contains('Path:audio') -and $bytes.Length -gt (2 + $hlen)) {
                $mp3.Write($bytes, 2 + $hlen, $bytes.Length - 2 - $hlen)
            }
        } elseif ($res.MessageType -eq 'Text') {
            $msg = [Text.Encoding]::UTF8.GetString($bytes)
            if ($msg.Contains('Path:audio.metadata')) {
                $idx = $msg.IndexOf("`r`n`r`n")
                if ($idx -ge 0) {
                    $json = $msg.Substring($idx + 4) | ConvertFrom-Json
                    foreach ($m in $json.Metadata) {
                        if ($m.Type -eq 'WordBoundary') {
                            $s = [double]$m.Data.Offset / 10000000.0
                            $d = [double]$m.Data.Duration / 10000000.0
                            $w = ($m.Data.text.Text -replace "[\r\n\t]", ' ')
                            $tsv.WriteLine(([string]::Format([Globalization.CultureInfo]::InvariantCulture, "{0:F4}`t{1:F4}`t{2}", $s, $s + $d, $w)))
                        }
                    }
                }
            } elseif ($msg.Contains('Path:turn.end')) {
                $done = $true
            }
        }
    }
} finally {
    $mp3.Dispose()
    $tsv.Dispose()
    if ($ws.State -eq 'Open') {
        try { $ws.CloseOutputAsync([System.Net.WebSockets.WebSocketCloseStatus]::NormalClosure, '', [Threading.CancellationToken]::None).GetAwaiter().GetResult() } catch {}
    }
    $ws.Dispose()
}
"#;
    let Ok(mut child) = std::process::Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-WindowStyle", "Hidden", "-Command", ps_script])
        .env("NOTECRAFT_EDGE_URL", &url)
        .env("NOTECRAFT_EDGE_REQ", &conn_id)
        .env("NOTECRAFT_EDGE_SSML", &ssml)
        .env("NOTECRAFT_TTS_MP3", mp3)
        .env("NOTECRAFT_TTS_TSV", tsv)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
    else {
        return false;
    };
    pid_slot.store(child.id(), Ordering::SeqCst);
    let ok = child.wait().is_ok_and(|s| s.success());
    pid_slot.store(0, Ordering::SeqCst);
    ok && !stop_flag.load(Ordering::SeqCst) && std::fs::metadata(mp3).is_ok_and(|m| m.len() > 256)
}

/// Synthesizes offline neural speech into `wav` using the cached Piper executable and model.
#[cfg(target_os = "windows")]
pub fn synthesize_piper_wav(model_name: &str, length_scale: &str, text: &str, wav: &Path, pid_slot: &AtomicU32, stop_flag: &AtomicBool) -> bool {
    use std::io::Write;
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    if stop_flag.load(Ordering::SeqCst) {
        return false;
    }
    let dir = piper_dir();
    let exe = dir.join("piper").join("piper.exe");
    let model = dir.join(model_name);
    if !exe.is_file() || !model.is_file() {
        return false;
    }
    let _ = std::fs::remove_file(wav);
    let Ok(mut child) = std::process::Command::new(&exe)
        .arg("--model")
        .arg(&model)
        .arg("--length_scale")
        .arg(length_scale)
        .arg("--output_file")
        .arg(wav)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
    else {
        return false;
    };
    pid_slot.store(child.id(), Ordering::SeqCst);
    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(text.as_bytes());
    }
    let ok = child.wait().is_ok_and(|s| s.success());
    pid_slot.store(0, Ordering::SeqCst);
    ok && !stop_flag.load(Ordering::SeqCst) && std::fs::metadata(wav).is_ok_and(|m| m.len() > 64)
}

/// Plays a synthesized MP3 or WAV file via hidden Windows media helpers, tracking `pid_slot`.
#[cfg(target_os = "windows")]
pub fn play_audio(path: &Path, is_wav: bool, pid_slot: &AtomicU32, stop_flag: &AtomicBool) -> bool {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    if stop_flag.load(Ordering::SeqCst) {
        return false;
    }
    let script = if is_wav {
        "(New-Object System.Media.SoundPlayer $env:NOTECRAFT_AUDIO).PlaySync()"
    } else {
        r#"
Add-Type -AssemblyName PresentationCore
$p = New-Object System.Windows.Media.MediaPlayer
$p.Open((New-Object System.Uri($env:NOTECRAFT_AUDIO)))
$t = 0
while (-not $p.NaturalDuration.HasTimeSpan -and $t -lt 100) {
    Start-Sleep -Milliseconds 50
    $t++
}
if ($p.NaturalDuration.HasTimeSpan) {
    $dur = $p.NaturalDuration.TimeSpan
    $p.Play()
    while ($p.Position -lt $dur) {
        Start-Sleep -Milliseconds 50
    }
}
$p.Close()
"#
    };
    let Ok(mut child) = std::process::Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-WindowStyle", "Hidden", "-STA", "-Command", script])
        .env("NOTECRAFT_AUDIO", path)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
    else {
        return false;
    };
    pid_slot.store(child.id(), Ordering::SeqCst);
    let ok = child.wait().is_ok_and(|s| s.success());
    pid_slot.store(0, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edge_sec_ms_gec_matches_known_v2_vector() {
        assert_eq!(edge_sec_ms_gec(1_700_000_000), "2CD23A2CA0B2595A-F3C846C370E05F9CC83ACC5F995C5F5D87A4D0F14CE8936A0F902E64099E38A2");
        let url = edge_wss_url(1_700_000_000, "abc");
        assert!(url.contains("ConnectionId=abc"));
        assert!(url.contains(&format!("Sec-MS-GEC-Version={EDGE_SEC_MS_GEC_VERSION}")));
    }

    #[test]
    fn edge_ssml_escapes_xml_characters() {
        let ssml = edge_ssml("fr-FR-DeniseNeural", "+0%", "a < b & \"c\" > 'd'");
        assert!(ssml.contains("fr-FR-DeniseNeural"));
        assert!(ssml.contains("a &lt; b &amp; &quot;c&quot; &gt; &apos;d&apos;"));
    }

    #[test]
    fn piper_urls_exist_for_all_supported_models() {
        for model in [
            "fr_FR-siwis-medium.onnx",
            "en_US-lessac-medium.onnx",
            "de_DE-thorsten-medium.onnx",
            "es_ES-davefx-medium.onnx",
            "it_IT-paola-medium.onnx",
        ] {
            let (onnx, cfg) = piper_voice_urls(model).expect("urls");
            assert!(onnx.starts_with("https://huggingface.co/rhasspy/piper-voices/"));
            assert_eq!(cfg, format!("{onnx}.json"));
        }
    }
}
