//! One low-delay LoadSamples/Prepare/AdvancePlaylist owner and its PCM ring.
//!
//! gamemd404700/4047B0 select one buffer at a time. Fill409880 invokes
//! AdvancePlaylist405AC0 while filling the initial ring and consumed quarters;
//! this is lookahead in PCM bytes, not a queued complete playlist pass.
//! Source: tools/input_oracle/gattling_loop.{py,json,meta.json,md}.

use super::{DecodedAudio, LoadedPlayback, LoadedSampleIndices, PlaybackDraws, SampleRng};
use crate::rules::sound_ini::control;
use rodio::Source;
use std::num::NonZero;
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, Weak};
use std::time::Duration;

/// Owns native+AC/+160/+1E0/+1E4. Admission and handles stay in the arbiter.
pub(super) struct PlaylistCursor {
    selected: LoadedSampleIndices,
    remaining: Vec<usize>,
    control: u32,
    delay_min: i32,
    loop_count: i32,
    iteration: i32,
    decay_used: bool,
    no_replay: bool,
}

impl PlaylistCursor {
    pub(super) fn new(
        selected: LoadedSampleIndices,
        control: u32,
        delay_min: i32,
        loop_count: i32,
    ) -> Self {
        let remaining = Vec::with_capacity(selected.middle.len());
        Self {
            selected,
            remaining,
            control,
            delay_min,
            loop_count,
            iteration: 0,
            decay_used: false,
            no_replay: false,
        }
    }

    /// Both4045D1 and404673 reset the same remaining list. Attack bypasses
    /// RandomRanged; SkipAttack reallocation selects one body here instead.
    pub(super) fn prepare(
        &mut self,
        rng: &mut impl SampleRng,
        plays_attack: bool,
    ) -> Option<usize> {
        self.remaining.clear();
        self.remaining.extend_from_slice(&self.selected.middle);
        // Prepare404700 does not clear flags0x10. Only allocation creates
        // an unused decay; another preparation must not replay it.
        if plays_attack && let Some(attack) = self.selected.attack {
            return Some(attack);
        }
        self.advance(rng)
    }

    pub(super) fn prevent_replay(&mut self) {
        self.no_replay = true;
    }

    pub(super) fn advance(&mut self, rng: &mut impl SampleRng) -> Option<usize> {
        if !self.no_replay && !self.remaining.is_empty() {
            let pick = if self.control & control::RANDOM != 0 {
                rng.ranged(0, self.remaining.len() as i32 - 1) as usize
            } else {
                0
            };
            return Some(self.remaining.remove(pick.min(self.remaining.len() - 1)));
        }
        // Original40483D..404879 checks LOOP before DECAY. This counter is
        // authoritative here; the arbiter must not spend passes in advance.
        if !self.no_replay
            && self.delay_min < super::arbiter::PREDELAY_FLOOR_MS
            && self.control & control::LOOP != 0
            && (self.loop_count == 0 || self.iteration < self.loop_count - 1)
        {
            self.iteration += 1;
            // Empty loaded bodies cannot sustain a zero-byte callback loop.
            if !self.selected.middle.is_empty() {
                return self.prepare(rng, false);
            }
        }
        if !self.decay_used
            && let Some(decay) = self.selected.decay
        {
            // Original40487E..40489A writes flags0x10 only when a
            // Control=decay buffer actually exists. A one-shot exhaustion
            // must not invent that lifecycle flag.
            self.decay_used = true;
            return Some(decay);
        }
        None
    }
}

/// Native40A4A6..40A551: round_up_1024((avgBytes*3 /100)*8), then four
/// quarters. The source rate, not its shifted playback frequency, is used.
fn quarter_samples(clip: &DecodedAudio) -> Option<usize> {
    let bytes_per_second = clip
        .sample_rate
        .checked_mul(u32::from(clip.native_frame_bytes))?;
    let nominal = (bytes_per_second.checked_mul(3)? / 100).checked_mul(8)?;
    let quarter_bytes = nominal.checked_add(1023)? & !1023;
    let frames = quarter_bytes.checked_div(u32::from(clip.native_frame_bytes))?;
    usize::try_from(frames.checked_mul(u32::from(clip.channels))?)
        .ok()
        .filter(|n| *n != 0)
}

struct RingState {
    loaded: LoadedPlayback,
    cursor: PlaylistCursor,
    rng: PlaybackDraws,
    current: Option<usize>,
    clip_offset: usize,
    ring: Vec<f32>,
    valid: [usize; 4],
    quarter_samples: usize,
    read_quarter: usize,
    first_pull: bool,
    cancelled: bool,
    format: (u32, u16, u16),
    observation: Option<Arc<PlaybackTrace>>,
    // Diagnostic provenance follows every actual ring write, including
    // replacement on release. It never supplies a playback decision.
    provenance: Option<Vec<usize>>,
}

impl RingState {
    fn fill(&mut self, start: usize, capacity: usize) -> usize {
        let mut copied = 0;
        let mut empty_selections = 0;
        while copied < capacity {
            let Some(index) = self.current else { break };
            let Some(clip) = self.loaded.clips.get(&index) else {
                empty_selections += 1;
                if empty_selections
                    > self.loaded.clips.len() + self.loaded.selected.middle.len() + 2
                {
                    // Missing retail payloads may not spin a device callback.
                    // Successful loaded paths never enter this guard.
                    self.current = None;
                    break;
                }
                self.current = self.cursor.advance(&mut self.rng);
                self.clip_offset = 0;
                continue;
            };
            let compatible =
                (clip.sample_rate, clip.channels, clip.native_frame_bytes) == self.format;
            let available = if compatible {
                clip.samples.len().saturating_sub(self.clip_offset)
            } else {
                // Preserve the previous shared player's mixed-format limit.
                // Stock Loop1/Loop2 are uniform; native format transitions
                // outside that corpus remain a separate required mechanism.
                log::warn!("SFX: dropped chained sample with mismatched PCM format");
                0
            };
            if available == 0 {
                empty_selections += 1;
                if empty_selections
                    > self.loaded.clips.len() + self.loaded.selected.middle.len() + 2
                {
                    self.current = None;
                    break;
                }
                self.current = self.cursor.advance(&mut self.rng);
                self.clip_offset = 0;
                continue;
            }
            let count = available.min(capacity - copied);
            if let Some(trace) = &self.observation {
                let slot = trace
                    .clips
                    .iter()
                    .position(|clip| clip.index == index)
                    .unwrap();
                if self.clip_offset == 0 {
                    trace.started(slot);
                }
                trace.clips[slot]
                    .filled_samples
                    .fetch_add(count, Ordering::Relaxed);
                self.provenance.as_mut().unwrap()[start + copied..start + copied + count]
                    .fill(slot);
            }
            self.ring[start + copied..start + copied + count]
                .copy_from_slice(&clip.samples[self.clip_offset..self.clip_offset + count]);
            self.clip_offset += count;
            copied += count;
            empty_selections = 0;
            // Original409BCA returns when capacity is exhausted. Selection
            // at an exact clip end waits for the next actual fill request.
        }
        self.ring[start + copied..start + capacity].fill(0.0);
        if let Some(provenance) = &mut self.provenance {
            provenance[start + copied..start + capacity].fill(usize::MAX);
        }
        copied
    }

    fn fill_initial(&mut self) {
        let copied = self.fill(0, self.ring.len());
        for (q, valid) in self.valid.iter_mut().enumerate() {
            *valid = copied
                .saturating_sub(q * self.quarter_samples)
                .min(self.quarter_samples);
        }
        self.read_quarter = 0;
        self.first_pull = true;
    }

    fn next_quarter(&mut self, target: &mut [f32], provenance: Option<&mut [usize]>) -> usize {
        if self.cancelled {
            return 0;
        }
        if self.first_pull {
            self.first_pull = false;
        } else {
            let previous = self.read_quarter;
            let ended = self.valid[previous] < self.quarter_samples;
            self.valid[previous] = self.fill(previous * self.quarter_samples, self.quarter_samples);
            // The native last refill also overwrites the consumed padded
            // quarter with silence. It cannot select another clip, but its
            // real buffer write remains observable before endpoint delivery.
            if ended {
                return 0;
            }
            self.read_quarter = (previous + 1) % 4;
        }
        let count = self.valid[self.read_quarter];
        let start = self.read_quarter * self.quarter_samples;
        if count == 0 {
            return 0;
        }
        // Original worker stops only once decoded_remaining becomes negative
        // on the quarter crossing. Keep the last quarter's zero padding.
        target.copy_from_slice(&self.ring[start..start + self.quarter_samples]);
        if let Some(target) = provenance {
            target.copy_from_slice(
                &self.provenance.as_ref().unwrap()[start..start + self.quarter_samples],
            );
        }
        self.quarter_samples
    }
}

const MAX_OBSERVED_CLIP_STARTS: usize = 128;

struct ClipCounters {
    index: usize,
    name: String,
    filled_samples: AtomicUsize,
    pulled_samples: AtomicUsize,
}

struct PlaybackTrace {
    clips: Vec<ClipCounters>,
    starts: [AtomicUsize; MAX_OBSERVED_CLIP_STARTS],
    start_count: AtomicUsize,
}

impl PlaybackTrace {
    fn started(&self, slot: usize) {
        // Fills are serialized by the existing output boundary gate. Publish
        // the slot before its count; snapshots also hold that same gate.
        let count = self.start_count.load(Ordering::Relaxed);
        if let Some(start) = self.starts.get(count) {
            start.store(slot, Ordering::Relaxed);
        }
        self.start_count.store(count + 1, Ordering::Release);
    }
}

#[derive(Clone, serde::Serialize)]
pub(super) struct PlaybackClipObservation {
    pub(super) name: String,
    pub(super) filled_samples: usize,
    pub(super) pulled_samples: usize,
}

pub(super) struct PlaybackObservationSnapshot {
    pub(super) resolved_samples: Vec<String>,
    pub(super) source_sample_count: usize,
    pub(super) clips: Vec<PlaybackClipObservation>,
    pub(super) source_state_alive: bool,
    pub(super) truncated: bool,
}

/// Passive counters outlive the source; a weak reference independently
/// witnesses whether its loaded clips, cursor and RNG capability still live.
#[derive(Clone)]
pub(super) struct PlaybackObservation {
    trace: Arc<PlaybackTrace>,
    state: Weak<SharedOutput>,
}

impl PlaybackObservation {
    pub(super) fn snapshot(&self) -> PlaybackObservationSnapshot {
        // A live fill publishes its copied counts before cache pulls. Hold
        // the existing fill gate while copying counters so a future refill
        // cannot make a newer pull exceed an older copied count in this row.
        // Once the weak reference expires every counter is stable.
        let state = self.state.upgrade();
        let _gate = state
            .as_ref()
            .map(|state| state.state.lock().expect("SFX boundary gate poisoned"));
        let starts = self.trace.start_count.load(Ordering::Acquire);
        let resolved_samples = self.trace.starts[..starts.min(MAX_OBSERVED_CLIP_STARTS)]
            .iter()
            .map(|slot| self.trace.clips[slot.load(Ordering::Relaxed)].name.clone())
            .collect();
        let clips: Vec<_> = self
            .trace
            .clips
            .iter()
            .map(|clip| PlaybackClipObservation {
                name: clip.name.clone(),
                filled_samples: clip.filled_samples.load(Ordering::Relaxed),
                pulled_samples: clip.pulled_samples.load(Ordering::Relaxed),
            })
            .collect();
        PlaybackObservationSnapshot {
            resolved_samples,
            source_sample_count: clips.iter().map(|clip| clip.filled_samples).sum(),
            clips,
            source_state_alive: state.is_some(),
            truncated: starts > MAX_OBSERVED_CLIP_STARTS,
        }
    }
}

struct SharedOutput {
    state: Mutex<RingState>,
    cancelled: AtomicBool,
    generation: AtomicU64,
    pan: AtomicI32,
}

/// Main-thread controls share the same boundary gate as buffer selection.
/// Lock order is output gate → one Main operation. No Main guard survives a
/// draw, asset loading, player submission or control acquisition.
#[derive(Clone)]
pub(super) struct PlaybackControl(Arc<SharedOutput>);

impl PlaybackControl {
    pub(super) fn cancel(&self) {
        let mut state = self.0.state.lock().expect("SFX boundary gate poisoned");
        state.cancelled = true;
        self.0.cancelled.store(true, Ordering::Release);
    }

    /// Native Release406060/Detach405FD0 bracket the live channel while
    /// mutating its event flags. The arbiter supplies that sole decision;
    /// the source gate makes it atomic with respect to quarter selection.
    pub(super) fn update_no_replay(&self, update_event: impl FnOnce() -> bool) {
        let mut state = self.0.state.lock().expect("SFX boundary gate poisoned");
        if update_event() {
            state.cursor.prevent_replay();
        }
    }

    pub(super) fn release_to_decay(&self) -> bool {
        let mut state = self.0.state.lock().expect("SFX boundary gate poisoned");
        if state.cancelled {
            return false;
        }
        state.cursor.prevent_replay();
        state.current = {
            let RingState { cursor, rng, .. } = &mut *state;
            cursor.advance(rng)
        };
        state.clip_offset = 0;
        state.fill_initial();
        self.0.generation.fetch_add(1, Ordering::Release);
        state.valid.iter().any(|n| *n != 0)
    }

    pub(super) fn set_pan(&self, pan: i32) {
        self.0.pan.store(pan, Ordering::Relaxed);
    }
}

/// One continuous rodio source. Ordinary sample pulls are lock-free; only
/// quarter refill and controls take the gate. The cache is storage, not a
/// second playlist or RNG cursor; a release epoch discards old cached PCM.
pub(super) struct RingSource {
    shared: Arc<SharedOutput>,
    cache: Vec<f32>,
    valid: usize,
    position: usize,
    generation: u64,
    channels: NonZero<u16>,
    sample_rate: NonZero<u32>,
    last_pan: i32,
    pan_gain: (f32, f32),
    observation: Option<Arc<PlaybackTrace>>,
    provenance: Option<Vec<usize>>,
}

impl RingSource {
    pub(super) fn new(
        loaded: LoadedPlayback,
        cursor: PlaylistCursor,
        initial: usize,
        rng: PlaybackDraws,
        shifted_rate: u32,
        pan: i32,
        observed_names: Option<&[String]>,
    ) -> Option<(Self, PlaybackControl)> {
        let clip = loaded.clips.get(&initial)?;
        let channels = NonZero::new(clip.channels)?;
        let sample_rate = NonZero::new(shifted_rate)?;
        let quarter = quarter_samples(clip)?;
        let format = (clip.sample_rate, clip.channels, clip.native_frame_bytes);
        let observation = observed_names.map(|names| {
            Arc::new(PlaybackTrace {
                clips: loaded
                    .clips
                    .keys()
                    .map(|&index| ClipCounters {
                        index,
                        name: names[index].clone(),
                        filled_samples: AtomicUsize::new(0),
                        pulled_samples: AtomicUsize::new(0),
                    })
                    .collect(),
                starts: std::array::from_fn(|_| AtomicUsize::new(usize::MAX)),
                start_count: AtomicUsize::new(0),
            })
        });
        let mut state = RingState {
            loaded,
            cursor,
            rng,
            current: Some(initial),
            clip_offset: 0,
            ring: vec![0.0; quarter.checked_mul(4)?],
            valid: [0; 4],
            quarter_samples: quarter,
            read_quarter: 0,
            first_pull: true,
            cancelled: false,
            format,
            observation: observation.clone(),
            provenance: observation.as_ref().map(|_| vec![usize::MAX; quarter * 4]),
        };
        // Native40A463 fills the complete ring synchronously at start. Later
        // callbacks refill consumed quarters on the same Main capability.
        state.fill_initial();
        let shared = Arc::new(SharedOutput {
            state: Mutex::new(state),
            cancelled: AtomicBool::new(false),
            generation: AtomicU64::new(0),
            pan: AtomicI32::new(pan),
        });
        let control = PlaybackControl(Arc::clone(&shared));
        Some((
            Self {
                shared,
                cache: vec![0.0; quarter],
                valid: 0,
                position: 0,
                generation: 0,
                channels,
                sample_rate,
                last_pan: pan,
                pan_gain: super::pan_channel_gains(pan),
                observation: observation.clone(),
                provenance: observation.map(|_| vec![usize::MAX; quarter]),
            },
            control,
        ))
    }

    pub(super) fn observation(&self) -> Option<PlaybackObservation> {
        self.observation.as_ref().map(|trace| PlaybackObservation {
            trace: Arc::clone(trace),
            state: Arc::downgrade(&self.shared),
        })
    }

    #[cfg(test)]
    pub(super) fn native_snapshot(&self) -> serde_json::Value {
        let state = self
            .shared
            .state
            .lock()
            .expect("SFX boundary gate poisoned");
        serde_json::json!({
            "ring": state.ring,
            "current": state.current,
            "clip_offset": state.clip_offset,
            "remaining": state.cursor.remaining,
            "iteration": state.cursor.iteration,
            "decay_used": state.cursor.decay_used,
            "no_replay": state.cursor.no_replay,
            "cancelled": state.cancelled,
            "quarter_samples": state.quarter_samples,
        })
    }

    #[cfg(test)]
    pub(super) fn fill_capacity_for_test(&self, start: usize, capacity: usize) {
        // Supplied component capacity in the native exact-end control.
        // Execute the production fill; this adapter makes no selection.
        self.shared
            .state
            .lock()
            .expect("SFX boundary gate poisoned")
            .fill(start, capacity);
    }
}

impl Iterator for RingSource {
    type Item = f32;
    fn next(&mut self) -> Option<f32> {
        if self.shared.cancelled.load(Ordering::Acquire) {
            return None;
        }
        let generation = self.shared.generation.load(Ordering::Acquire);
        if self.generation != generation {
            self.generation = generation;
            self.valid = 0;
            self.position = 0;
        }
        if self.position == self.valid {
            let mut state = self
                .shared
                .state
                .lock()
                .expect("SFX boundary gate poisoned");
            // Pair the fetched quarter with its reset epoch under the gate;
            // otherwise release during acquisition could skip new decay PCM.
            self.generation = self.shared.generation.load(Ordering::Acquire);
            self.valid = state.next_quarter(&mut self.cache, self.provenance.as_deref_mut());
            self.position = 0;
            if self.valid == 0 {
                return None;
            }
        }
        let value = self.cache[self.position];
        let channel = self.position % usize::from(self.channels.get());
        if let Some(trace) = &self.observation {
            let slot = self.provenance.as_ref().unwrap()[self.position];
            if slot != usize::MAX {
                trace.clips[slot]
                    .pulled_samples
                    .fetch_add(1, Ordering::Relaxed);
            }
        }
        self.position += 1;
        let pan = self.shared.pan.load(Ordering::Relaxed);
        if pan != self.last_pan {
            self.last_pan = pan;
            self.pan_gain = super::pan_channel_gains(pan);
        }
        let (left, right) = self.pan_gain;
        Some(value * if channel == 0 { left } else { right })
    }
}

impl Source for RingSource {
    fn current_span_len(&self) -> Option<usize> {
        // rodio0.22.2 Source::current_span_len permits Some(0) only at
        // exhaustion. PCM quarters do not change this source's format;
        // report one span until Iterator::next delivers the real endpoint.
        None
    }
    fn channels(&self) -> NonZero<u16> {
        self.channels
    }
    fn sample_rate(&self) -> NonZero<u32> {
        self.sample_rate
    }
    fn total_duration(&self) -> Option<Duration> {
        None
    }
}
