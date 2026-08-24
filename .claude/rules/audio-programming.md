---
paths:
  - "__agent_only_never_match_at_startup__/**"
---

# Audio Programming

A reference for evaluating audio code from a real-time safety, DSP correctness, and architecture lens. Used by the `audio-programming` subagent.

The scope: **the audio thread / real-time constraints**, DSP fundamentals (sampling, aliasing, FIR/IIR filters, biquads, convolution, FFT, oversampling), audio architectures (PureData / Max-MSP graph-based; JUCE C++ framework; SuperCollider; Web Audio API / AudioWorklet; AVAudioEngine / Oboe / AAudio), plugin formats (VST3, AudioUnit, AAX, LV2, CLAP), MIDI / OSC, spatial audio.

Distinct from:
- **`concurrency`**: general intra-machine concurrency. Audio has the same concerns but with a fixed real-time deadline measured in milliseconds; the patterns differ.
- **`performance`**: general performance. Audio's perf budget is the audio callback's deadline -- absolute, not relative.
- **`i18n`**: text concerns. Audio has metadata but i18n is not the lens.

The core thesis: **audio code differs from general systems code in one load-bearing way: the audio thread has a hard real-time deadline measured in milliseconds, and missing it produces an immediately audible artifact.** A web server at 12ms when budget is 10ms has a latency regression; an audio callback at 12ms when budget is 10.67ms has a click, a pop, or a buffer underrun the user hears.

This single fact reorganizes the engineering priorities. **Memory allocation, locks, system calls, and exceptions are individually fine in most code; on the audio thread they are bugs.** Most "rules" in audio programming are downstream of "do not miss the deadline, ever, even when the OS is busy, the user is moving the mouse, garbage collection is firing, the file system is paging in, or another core is hammering the cache."

The cultural consequence: audio engineers are deeply skeptical of abstractions whose runtime cost is non-obvious. STL containers with hidden allocations, smart pointers with reference-count atomics, virtual dispatch, exceptions, lazy init, lock-based synchronization, "modern" idioms generally are evaluated by their behavior under the deadline, not by their syntactic elegance. **The Ross Bencina "no" list** ("Real-time audio programming 101: time waits for nothing") is the canonical articulation.

---

## The audio thread: Bencina's "no" list

Anything that may block, allocate, lock, or take unbounded time is forbidden on the audio thread:

- **No allocation**: `new`, `malloc`, `std::vector::push_back` past capacity, `std::string` operations, `std::shared_ptr` construction, `std::function` from non-trivial lambdas, any container constructor or growth.
- **No locks**: `std::mutex::lock`, `std::condition_variable::wait`, `pthread_mutex_lock`, `[NSLock lock]`. Also indirect: `std::cout` (locks iostream sync), `printf` (locks stdio), `NSLog` (locks log system), `std::shared_ptr` destruction (atomic, but contention can serialize on some platforms).
- **No system calls**: file I/O (`fopen`, `read`, `write`, `fread`), network (`recv`, `send`), `sleep`, `usleep`, `nanosleep`, `select`, `poll`, `kqueue`, `epoll_wait`.
- **No exceptions**: `throw`, `dynamic_cast` (uses RTTI), `std::vector::at` (throws), `std::stoi` (throws on parse error). Stack unwinding allocates.
- **No per-sample virtual calls**: `virtual` invoked inside the per-sample loop. Virtualize at block boundaries, not per sample.
- **No logging**: `std::cout`, `printf`, `OutputDebugStringA`, `NSLog`, `os_log`, `tracing::info!`, `slog::info!`. All allocate / lock / system-call.
- **No GC-triggering allocation in managed-language audio**: C# allocating reference types on the audio thread, JavaScript allocating in AudioWorklet, Swift creating ARC-counted objects (`String`, `Array`, class instances).

The corresponding **"yes" list** for what enables real-time safety:

- **Pre-allocate everything in init / setup** (`prepareToPlay`, constructor, `AudioWorkletProcessor.constructor`).
- **Lock-free communication** with non-audio threads: SPSC ring buffers for messages, double-buffered state for parameters.
- **Atomic flags** for state transitions, not locks. `std::atomic<bool>` for "should we mute," etc.
- **Parameter smoothing** at the audio rate to avoid clicks on parameter changes.
- **Denormal-flush mode** (FTZ / DAZ on x86; default-NaN / default-denormal-zero on ARM). Set at top of every `processBlock`.
- **State reset** on `prepareToPlay` / `prepare` / `start`: zero filter states, reset phase accumulators, clear ring buffers.

---

## DSP fundamentals (what the reviewer must recognize)

### Sampling and the Nyquist theorem

A continuous signal sampled at rate `fs` can perfectly represent frequencies up to `fs/2`. Frequencies above `fs/2` **alias** -- fold back into the audible range. The cardinal DSP failure mode.

Common rates: 44.1 kHz (CD, consumer), 48 kHz (pro, video, broadcast), 96 kHz / 192 kHz (high-resolution; mostly headroom for processing), 8 kHz / 16 kHz (telephony, speech recognition).

**Flag**: sample rate hardcoded (`44100.0` or `48000.0` baked into filter coefficient calculations); no resampling path between sources at different rates; assumption of a specific rate in DSP that should be rate-aware.

### Bit depth and headroom

Internal processing should be 32-bit float or 64-bit double (no quantization in math). 0 dBFS is the top; signals approaching it must be managed (clipping, peak limiting).

### Filters: FIR vs IIR, biquads, stability

- **FIR (Finite Impulse Response)**: linear-phase, always stable, computationally expensive at large taps.
- **IIR (Infinite Impulse Response)**: recursive, cheaper, can be unstable, non-linear phase.
- **Biquad** (2nd-order IIR section): the basic building block. Robert Bristow-Johnson's *Audio EQ Cookbook* is the canonical filter-design reference.
- **Filter denormal handling**: tiny outputs over long ringing tails produce denormals; CPU can spike 100x on a quiet signal.

**Stability**: pole magnitudes must be < 1. High-Q biquads near Nyquist, resonant filters at zero cutoff, gain values causing pole-magnitude > 1 all explode.

**Flag**: filter coefficients computed at construction without checking parameter ranges; filter coefficient changes applied instantaneously (causing state pop); no anti-denormal protection.

### Convolution

- Direct convolution: O(N×M). Too slow for impulse responses with M > 128 samples.
- FFT-based convolution: O(N log M) but introduces latency = M samples.
- Partitioned convolution: split impulse response; combine direct (short prefix) + FFT (longer tail) for zero-latency long IRs.

### FFT and windowing

- DFT: O(N²). FFT (Cooley-Tukey 1965): O(N log N).
- **Windowing**: rectangular leaks; Hann, Hamming, Blackman, Kaiser balance leakage and resolution.
- **Overlap-add / overlap-save**: streaming FFT processing.
- **Phase vocoder**: time-stretching / pitch-shifting via STFT manipulation.

### Oversampling

For nonlinear processes (distortion, saturation, hard clipping, wave shaping): the nonlinearity generates harmonics above the source's Nyquist; those alias. Oversample 2x / 4x / 8x; process; lowpass filter; decimate.

Alternatives: BLIT, BLEP, ADAA (Bilżymowicz) -- alias-suppressed waveforms / nonlinearities for specific cases.

**Flag**: distortion / saturation / hard clipping at base rate (audible aliasing on high-frequency input); naive sawtooth / square as `2 * (phase - floor(phase)) - 1` (aliases heavily).

### Spatial audio

- **Equal-power panning** (cosine / sine) vs linear: equal-power preserves loudness at center.
- **VBAP** (Pulkki): Vector Base Amplitude Panning for surround.
- **Ambisonics**: spherical-harmonic encoding (B-format, higher-order).
- **HRTF / binaural**: head-related transfer functions for 3D over headphones.

---

## The audio thread discipline (deep dive)

### The callback contract

The audio callback is invoked at intervals dictated by buffer size + sample rate. Typical buffer sizes: 64-512 samples. At 48kHz, 64 samples = 1.33ms; 512 = 10.67ms.

**The callback MUST return before the deadline** or audio drops out (xrun on Linux, dropout on macOS / Windows).

The callback runs on a high-priority real-time thread, different priority class than UI / general worker.

### Denormal handling

Denormal floats (subnormals near zero) trigger microcode handling on most CPUs, ~100x slower than normal floats. IIR filter tails, reverb decays, delay-line ringing all produce them.

**Solutions**:
- **FTZ (Flush To Zero) / DAZ (Denormals Are Zero)** on x86: SSE control bits. JUCE's `juce::ScopedNoDenormals` sets these at the top of `processBlock`.
- **Default-NaN / FZ on ARM**: similar.
- **Anti-denormal injection**: add a tiny DC offset (e.g., `1e-20`) to filter states each block.

**Flag**: IIR filters / long delays / reverb tails without FTZ/DAZ; no `ScopedNoDenormals` in JUCE plugins; manual filter implementations without denormal handling.

### Parameter changes

Raw parameter assignment produces **clicks** on every knob movement. The fix: **parameter smoothing** -- ramp the parameter at audio rate over typically 5-20ms.

JUCE's `SmoothedValue` template handles this. Custom implementations need: target value, current value, increment per sample, sample count remaining.

**Flag**: raw parameter assignment in `processBlock`; smoothing time < 1ms (still clicks on large jumps); smoothing time > 50ms (feels unresponsive); no smoothing reset on `prepareToPlay` (first block plays previous session's ramp); coefficient changes applied instantly without crossfade or ramp.

### Cross-thread state

- **Lock-free SPSC queue** (single-producer single-consumer) for UI → audio messages.
- **Double-buffered state**: UI thread writes to buffer A; audio thread reads buffer B; atomic swap on transitions.
- **`std::atomic<T>` for parameters**: only trivially-copyable types up to 8 bytes (or 16 with double-CAS support); complex types fall back to locking under the hood.
- **Memory barriers on lock-free queues**: required for correctness on weak-memory architectures (ARM).

**Flag**: `std::mutex` shared between UI and audio thread (priority inversion); `std::atomic<ComplexType>` for non-trivial types; cross-thread pointer to non-owning data (UI frees memory audio thread still references); no memory barrier on lock-free queue.

---

## Audio architectures

### Pure Data and Max/MSP (graph-based)

Miller Puckette designed both. Max-MSP commercial (Cycling '74); Pure Data open. **Signal-flow programming**: boxes connected by patches; data flows along edges. Patches as the source-code unit; abstractions / subpatches for modularity.

**Sample-accurate** at signal rate (objects with `~` suffix); control rate (event scheduling) separate.

### JUCE (the commercial standard)

Julian Storer / Raw Material Software. Most commercial VST / AU plugins use JUCE. C++ framework with:
- **Plugin and standalone targets** from one codebase.
- **VST3 / AudioUnit / AAX / LV2 / standalone** export.
- **Projucer** project generator (and recent IDE integration).
- **DSP module**: ready-made filters, FFTs, oscillators, oversampling, IIR / FIR, NoiseGate, Compressor.
- **AudioProcessorGraph**: in-app audio graph.
- **`AudioBuffer<float>` and `AudioBuffer<double>`**: planar channel arrays.

JUCE-specific patterns: `juce::AudioProcessor::prepareToPlay`, `processBlock`, `releaseResources`. `getNumInputChannels()`, `getNumOutputChannels()`, `setBusesLayout`.

### Faust (functional DSP)

Letz / Orlarey / Fober at Grame. Functional DSP language compiling to C++, JS, WASM, Rust, etc. Source-to-source: write DSP once; generate VST / Web Audio / native.

Block-diagram syntax; signal-flow composition.

### Web Audio API

Chris Rogers / Chris Wilson (Google) original spec; Paul Adenot (Mozilla) current editor.

`AudioContext` + `AudioNode` graph. Built-in nodes: `OscillatorNode`, `BiquadFilterNode`, `PannerNode`, `ConvolverNode`, `DynamicsCompressorNode`, `GainNode`, `DelayNode`, `AnalyserNode`, `ChannelSplitterNode`, `ChannelMergerNode`.

**AudioWorklet (2018)**: custom DSP in JS / WASM running on the audio thread. Replaced `ScriptProcessorNode` (deprecated; ran on main thread; terrible).

**OfflineAudioContext**: render audio faster than real-time.

**Flag in Web Audio**: `ScriptProcessorNode` in new code (use AudioWorklet); `AudioWorkletProcessor` allocating per render (128 samples × ~344 callbacks/sec = significant GC); `postMessage` from `AudioWorkletProcessor` every render (allocates; use SharedArrayBuffer + atomics for high-rate); main-thread DSP via `setTimeout`; `AudioBuffer.getChannelData()` modified after `start()` (spec says decoupled); blocking `decodeAudioData` on main thread.

### Native mobile

- **iOS / macOS**: AVAudioEngine (high level), AudioUnit (low level, AUv3 for sandboxed plugins).
- **Android**: AAudio / Oboe (high-performance C/C++ APIs). Don Turner at Google. Replaces older OpenSL ES.

**Flag**: Java / Kotlin audio code for low-latency on Android (use Oboe); OpenSL ES in new code (use Oboe wrapper); iOS audio session category wrong (`AVAudioSessionCategoryAmbient` when should be `Playback` or `PlayAndRecord`); missing `UIBackgroundModes` `audio` for background-audio apps.

### Cross-platform libraries

- **PortAudio** (Ross Bencina, Phil Burk): C audio I/O across platforms.
- **RtAudio** (Gary Scavone): C++ alternative.
- **miniaudio** (David Reid): single-header C; very accessible.
- **libsoundio** (Andrew Kelley, Zig author): cross-platform audio I/O.

### Server / sound design

- **SuperCollider**: client/server architecture; `sclang` language; `scsynth` audio engine.
- **CSound**: original (1985); FORTRAN-derived score/orchestra model.
- **JACK Audio Connection Kit** (Letz et al.): Linux pro audio routing; macOS / Windows support.

---

## Plugin formats

### VST3 (Steinberg, 2008)

C++; component-based. Side-chain inputs, parameter automation, MIDI 2.0 (in VST 3.7). License: GPL or proprietary Steinberg.

### AudioUnit (Apple)

C / Objective-C / Swift. macOS and iOS (AUv3 for iOS, sandboxed). Built into Logic, GarageBand, Final Cut Pro X.

### AAX (Avid Pro Tools)

C++. Pro Tools-only; commercial license fee from Avid.

### LV2

David Robillard. C; URI-based extension model. Open, free. Used by Ardour, Carla.

### CLAP (2022)

Bitwig + u-he. C; modern alternative. Free, MIT license. Recent traction: Bitwig, Reaper, Bitwig support natively.

### Cross-format concerns

- **Bypass behavior**: latency-compensated; gain-matched. Non-compensated bypass produces comb filtering on parallel routing.
- **Parameter automation**: smoothing required to avoid zipper noise.
- **Side-chain inputs**: routing complexity; not always active.
- **Latency reporting**: must match actual latency for DAW compensation.
- **State save/load**: preset format; backward compatibility across plugin versions.
- **Garbage in first buffer**: filter states must be zeroed at `prepareToPlay` / `prepare`.

**Flag**: latency not reported correctly; latency not updated on internal change (FFT block size, oversampling factor); bypass not latency-compensated; state serialization not version-aware; sample-rate change not handled; plugin-format-specific code in core DSP (complicates ports); MIDI events lost; tempo / time-signature changes mid-block not handled.

---

## MIDI and OSC

### MIDI 1.0 (1983)

7-bit, 31250 baud serial. Channel / note / velocity / control change / program change / pitchbend / aftertouch. 16 channels per cable.

### MIDI 2.0 (2020)

32-bit resolution; bidirectional; profile / property exchange. Adopting slowly; ~2024-25 mainstream DAW support emerging. Backward compatible via translation.

### OSC (Open Sound Control)

Matt Wright / CNMAT (1997). IP / UDP; symbolic addresses (`/synth/freq`); typed arguments. Higher resolution than MIDI 1.0; not standardized in hardware.

---

## Anti-pattern catalog

### Audio-thread violations (blocker)

- Allocation in the callback.
- Lock in the callback.
- System call in the callback.
- C++ exception in the callback.
- Per-sample virtual call.
- Logging in the callback.
- GC-triggering allocation in managed-language audio.

### Denormal handling
- No FTZ/DAZ in code with IIR / long delays / reverb tails.
- No anti-denormal injection without FTZ/DAZ.
- FTZ set only in init, not in every `processBlock`.

### Parameter changes
- Raw assignment without smoothing.
- Smoothing time < 1ms or > 50ms.
- No smoothing reset on init.
- State pop on filter coefficient change without crossfade or ramp.

### Cross-thread state
- Mutex shared between UI and audio.
- `std::atomic<ComplexType>` for non-trivial types.
- Cross-thread pointer to non-owning data.
- No memory barrier on lock-free queue.

### DSP correctness
- Aliasing in nonlinear processing without oversampling.
- Naive band-limited oscillator (sawtooth as floor, square as sign).
- Filter instability at parameter edges.
- No DC blocker on output of asymmetric nonlinearities.
- Phase issues in parallel filters.
- Unbounded feedback or recursion.
- Sample rate hardcoded.
- Buffer size assumed fixed.
- Channel layout assumed stereo.
- Interleaved vs planar buffer confusion.

### Plugin-format compliance
- Latency not reported correctly.
- Latency not updated on internal change.
- Bypass not latency-compensated.
- State serialization not version-aware.
- Garbage in first buffer (filter states not zeroed).
- Loud pop on preset change.
- Sample-rate change not handled.
- Plugin-format-specific code in core DSP.
- MIDI events lost.
- Tempo / time-signature changes mid-block not handled.
- Sidechain routing mishandled.

### Web Audio specific
- `ScriptProcessorNode` in new code.
- AudioWorklet processor allocating per render.
- `postMessage` from `AudioWorkletProcessor` every render.
- Main thread doing DSP.
- `AudioBuffer.getChannelData()` modified after `start()`.
- Blocking `decodeAudioData` on main thread.

### Mobile-specific
- Java / Kotlin audio code on Android for low-latency.
- OpenSL ES used directly in new code.
- iOS audio session category wrong.
- Background audio not configured in Info.plist.
- Buffer size not matching device optimal.

### General hygiene
- Magic constants for sample rate / channel count / buffer size.
- No null check on channel pointers.
- No handling of `prepareToPlay` being called multiple times.
- No handling of `releaseResources`.
- Plugin not thread-safe between `processBlock` and `setStateInformation`.

---

## What is NOT an audio-programming finding

- **General concurrency** that's not on the audio thread (route to `concurrency`).
- **General performance** unrelated to audio's real-time constraint (route to `performance`).
- **Audio file I/O design** at the higher level (route to `system-design`).
- **Audio UX / mixing-engineer-perspective concerns** (out of scope).
- **Game-audio middleware** (FMOD, Wwise) integration unless it touches the audio thread.
- **Speech-to-text / text-to-speech** as services (not real-time DSP).

---

## Severity calibration

Per `panel-contract.md`:

- **blocker**: audio-thread violation with reachable trigger producing audible artifacts (allocation, lock, system call, exception, log in the callback). Plugin not loading. State corruption. Filter instability at default settings.
- **major**: DSP correctness bug (aliasing in default settings, instability at parameter limits, denormals on quiet signals, no parameter smoothing). Plugin-format compliance gap (latency wrong, bypass wrong). Sample-rate or buffer-size hardcoded. AudioWorkletProcessor allocating per render.
- **minor**: hygiene (magic constants, missing setup-time checks, channel-layout assumptions in unlikely cases).
- **nit**: style choices that don't affect audio output.
- **insight**: structural -- "this whole DSP path could be expressed in `juce::dsp::Oversampling`," "this AudioWorklet would benefit from WASM," "this state machine reduces to a parameter ramp."

Confidence: high when verifiable from code structure (`std::vector::push_back` inside `processBlock`); medium when requires runtime analysis ("this filter might be unstable at high Q"). The agent should suggest a test rather than assert a bug when uncertain.

---

## Process for the audio-programming agent

1. **Identify the surface.** Audio thread (callback, `processBlock`, `AudioWorkletProcessor.process`)? Setup code (`prepareToPlay`, init)? UI code? File I/O? Different rules apply.
2. **For audio-thread code**: walk the Bencina "no" list rigorously. Every allocation, lock, system call, exception, log, per-sample virtual call is a finding.
3. **For setup code**: relaxed rules; normal allocation, locking, exceptions are fine.
4. **Walk DSP correctness**: filter stability, aliasing, denormals, parameter smoothing, state reset, DC blocking, sample-rate awareness, channel-layout correctness.
5. **Walk plugin-format compliance** (if applicable): latency, bypass, state serialization, parameter automation, sidechain routing.
6. **Walk architecture**: setup vs runtime separation, lock-free communication between threads, buffer ownership, resource lifecycle.
7. **Route to other lenses**: general concurrency → `concurrency`; general perf → `performance`; data-flow architecture → `system-design`.
8. **Stay read-only.**
