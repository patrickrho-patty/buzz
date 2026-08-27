// Shared constants for the TTS pipeline (`tts.rs` split-out).
//
// Included back into `tts.rs` via `#[path]` — this file exists only to
// keep module sizes under the repository size gate.

// ── Constants ─────────────────────────────────────────────────────────────────

/// Maximum number of queued text items.
/// Prevents unbounded accumulation when the agent produces text faster than
/// TTS can play it. Excess items are dropped with a warning.
const TEXT_QUEUE_DEPTH: usize = 8;

/// How long the worker waits before checking the shutdown flag.
const RECV_TIMEOUT: Duration = Duration::from_millis(100);
/// Poll interval of the barge-in monitor thread. Bounds flag-to-silence
/// latency: a cancel is noticed within one tick, and rodio's internal
/// `periodic_access` wrapper stops the in-flight source within a further
/// ~5 ms — so playing audio dies ~15 ms after the flag is set, even while
/// the worker is blocked inside `synth_chunk`.
const MONITOR_TICK: Duration = Duration::from_millis(10);
const SPEAKER_ACTIVITY_TICK: Duration = Duration::from_millis(50);
const AUDIO_PRIME_TIMEOUT: Duration = Duration::from_secs(2);

/// Pocket TTS is a one-step consistency model, not diffusion. Kept for API compat.
const SYNTH_STEPS: usize = 1;

/// Fade-out length in samples (8 ms at 24 kHz ≈ 192 samples).
///
/// Applied only at the *end* of each synthesised sentence to eliminate the
/// click that would otherwise occur when a non-zero waveform terminates
/// abruptly. **No fade-in is applied** — see `apply_fade_out` for why preserving
/// the leading waveform is important.
const FADE_OUT_SAMPLES: usize = (SAMPLE_RATE as f64 * 0.008) as usize;

/// rodio 0.22.2 bootstraps `UniformSourceIterator` when a source is added to
/// the mixer (`conversions/uniform.rs:49-66`). Its empty queue's 512-sample
/// span (`queue.rs::SourcesQueueInput::new`) can therefore retain placeholder
/// format metadata until the next span. The lead-in covers that whole span,
/// rounded up to the next millisecond, while preserving the product's existing
/// 20 ms quiet ramp-up. Continuously queued chunks receive no synthetic padding.
const SAMPLES_PER_MS: usize = SAMPLE_RATE as usize / 1_000;
const PRODUCT_RAMP_UP_MS: usize = 20;
const PRODUCT_RAMP_UP_SAMPLES: usize = PRODUCT_RAMP_UP_MS * SAMPLES_PER_MS;
const RODIO_ADD_BOOTSTRAP_SPAN_SAMPLES: usize = 512;
const RODIO_ADD_BOOTSTRAP_CUSHION_MS: usize =
    RODIO_ADD_BOOTSTRAP_SPAN_SAMPLES.div_ceil(SAMPLES_PER_MS);
const SENTENCE_LEAD_IN_SAMPLES: usize = {
    let bootstrap_cushion = RODIO_ADD_BOOTSTRAP_CUSHION_MS * SAMPLES_PER_MS;
    if PRODUCT_RAMP_UP_SAMPLES > bootstrap_cushion {
        PRODUCT_RAMP_UP_SAMPLES
    } else {
        bootstrap_cushion
    }
};

type WorkerControlState = (
    Arc<AtomicBool>,
    Arc<AtomicBool>,
    WorkerCancelSignals,
    SpeakerGenerations,
    ActiveSpeaker,
    SpeakerCancellation,
    PlaybackProbe,
);
