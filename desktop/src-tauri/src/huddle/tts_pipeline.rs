// Public `TtsPipeline` handle split out of `tts.rs` (size-gate part-file).

// ── Public pipeline handle ────────────────────────────────────────────────────

/// Handle to the running TTS pipeline.
///
/// Not Clone — wrap in `Arc` to share across threads.
#[derive(Debug)]
pub struct TtsPipeline {
    /// Send preprocessed text into the pipeline.
    text_tx: SyncSender<QueuedText>,
    /// `true` while the agent is speaking. Shared with the STT pipeline for gating.
    #[allow(dead_code)]
    pub tts_active: Arc<AtomicBool>,
    /// Signals the worker thread to stop.
    shutdown: Arc<AtomicBool>,
    /// Cancel flag: worker drains the queue and stops current playback.
    /// Kept alive here so the Arc isn't dropped — the worker holds a clone.
    #[allow(dead_code)]
    cancel: Arc<AtomicBool>,
    human_floor: HumanFloor,
    /// Internal cancellation used only for voice changes. Kept separate so a
    /// concurrent human barge-in always clears every queued message.
    voice_cancel: Arc<AtomicBool>,
    /// Selected manifest voice. The worker reloads only the lightweight style
    /// when this changes; the warmed Pocket engine and audio player stay alive.
    voice: Arc<Mutex<String>>,
    /// Tags messages so a voice change drops only pre-change queue entries.
    voice_generation: Arc<AtomicU64>,
    /// Per-agent generations let removal invalidate that agent's queued and
    /// in-flight text without poisoning speech queued after the agent rejoins.
    speaker_generations: SpeakerGenerations,
    /// Speaker whose audio currently owns the shared player queue.
    active_speaker: ActiveSpeaker,
    /// Targeted cancellation used when an agent leaves the huddle.
    speaker_cancel: SpeakerCancellation,
    /// Shared player handle used to reject Stop clicks after playback drains.
    playback_probe: PlaybackProbe,
    /// Completed after the worker drains pre-change text and installs the new style.
    voice_change_ack: VoiceChangeAck,
    /// Worker thread handle — taken on drop to join cleanly.
    thread: Option<thread::JoinHandle<()>>,
}

impl TtsPipeline {
    /// Spawn the TTS pipeline thread with a manifest-backed voice name.
    ///
    /// `cancel` is shared with STT for barge-in. The same handle survives voice
    /// changes so the warmed Pocket engine is retained.
    pub fn new_with_voice(
        model_dir: PathBuf,
        tts_active: Arc<AtomicBool>,
        cancel: Arc<AtomicBool>,
        human_floor: HumanFloor,
        voice: &str,
        output_device: Option<String>,
        activity_app: Option<tauri::AppHandle>,
    ) -> Result<Self, String> {
        let (text_tx, text_rx) = mpsc::sync_channel::<QueuedText>(TEXT_QUEUE_DEPTH);
        let shutdown = Arc::new(AtomicBool::new(false));
        // cancel is passed in from HuddleState.tts_cancel — shared with remote
        // participant interruption and the push-to-talk shortcut.

        let shutdown_worker = Arc::clone(&shutdown);
        let cancel_worker = Arc::clone(&cancel);
        let worker_human_floor = human_floor.clone();
        let voice_cancel = Arc::new(AtomicBool::new(false));
        let worker_voice_cancel = Arc::clone(&voice_cancel);
        let tts_active_worker = Arc::clone(&tts_active);
        let voice = Arc::new(Mutex::new(voice.to_string()));
        let voice_worker = Arc::clone(&voice);
        let voice_generation = Arc::new(AtomicU64::new(1));
        let worker_voice_generation = Arc::clone(&voice_generation);
        let speaker_generations = Arc::new(Mutex::new(HashMap::new()));
        let worker_speaker_generations = Arc::clone(&speaker_generations);
        let active_speaker = Arc::new(Mutex::new(None));
        let worker_active_speaker = Arc::clone(&active_speaker);
        let speaker_cancel = Arc::new(Mutex::new(None));
        let worker_speaker_cancel = Arc::clone(&speaker_cancel);
        let playback_probe = PlaybackProbe::new();
        let worker_playback_probe = playback_probe.clone();
        let voice_change_ack = Arc::new(Mutex::new(None));
        let worker_voice_change_ack = Arc::clone(&voice_change_ack);
        let model_dir_worker = model_dir.clone();
        let (startup_tx, startup_rx) = mpsc::sync_channel(1);

        let handle = thread::Builder::new()
            .name("tts-worker".into())
            .spawn(move || {
                tts_worker(
                    model_dir_worker,
                    (
                        voice_worker,
                        worker_voice_generation,
                        worker_voice_change_ack,
                    ),
                    text_rx,
                    worker_human_floor,
                    (
                        tts_active_worker,
                        shutdown_worker,
                        (cancel_worker, worker_voice_cancel),
                        worker_speaker_generations,
                        worker_active_speaker,
                        worker_speaker_cancel,
                        worker_playback_probe,
                    ),
                    output_device,
                    activity_app,
                    startup_tx,
                )
            })
            .map_err(|e| format!("failed to spawn tts-worker thread: {e}"))?;
        let handle = await_worker_startup(handle, startup_rx)?;

        Ok(Self {
            text_tx,
            tts_active,
            shutdown,
            cancel,
            human_floor,
            voice_cancel,
            voice,
            voice_generation,
            speaker_generations,
            active_speaker,
            speaker_cancel,
            playback_probe,
            voice_change_ack,
            thread: Some(handle),
        })
    }
}

impl Drop for TtsPipeline {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::Release);
        // Dropping `text_tx` unblocks the worker's recv_timeout loop.
        // Join to ensure the audio thread exits cleanly.
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}
