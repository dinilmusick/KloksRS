//! Portable deterministic clock from Kloks package audio-midi clock/clock.ts.
//! Source pin: 67de26dd39161a0bc642038fdca75dcdfee8fae7.
//! The caller owns the scheduler and calls poll() with monotonic milliseconds.
//! This module never spawns timers, threads, resident agents or background work.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClockRunState { Stopped, Running, Paused }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MissedTickPolicy { CatchUp, Skip }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClockError { InvalidTempo, InvalidResolution, InvalidSignature,
    InvalidCatchUpLimit, InvalidTime, Disposed }

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ClockOptions {
    pub tempo: f64,
    pub resolution: u16,
    pub numerator: u8,
    pub denominator: u8,
    pub missed_tick_policy: MissedTickPolicy,
    pub max_catch_up_ticks: u32,
}
impl Default for ClockOptions {
    fn default() -> Self {
        Self { tempo: 120.0, resolution: 24, numerator: 4, denominator: 4,
            missed_tick_policy: MissedTickPolicy::CatchUp, max_catch_up_ticks: 1024 }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ClockPosition {
    pub state: ClockRunState,
    pub tempo: f64,
    pub resolution: u16,
    pub beat: f64,
    pub bar: u64,
    pub beat_in_bar: f64,
    pub tick: u64,
    pub subdivision: u16,
    pub elapsed_ms: f64,
    pub time_signature: (u8, u8),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ClockTick {
    pub position: ClockPosition,
    pub scheduled_beat: f64,
    pub emitted_at: f64,
    pub missed_ticks: u64,
}

#[derive(Clone, Debug)]
pub struct ClockService {
    options: ClockOptions,
    state: ClockRunState,
    anchor_time: f64,
    anchor_beat: f64,
    elapsed_at_anchor: f64,
    last_tick: u64,
    disposed: bool,
}

fn validate_time(now: f64) -> Result<(), ClockError> {
    if !now.is_finite() || now < 0.0 { Err(ClockError::InvalidTime) } else { Ok(()) }
}
fn validate_tempo(tempo: f64) -> Result<(), ClockError> {
    if !tempo.is_finite() || tempo <= 0.0 || tempo > 1000.0 {
        Err(ClockError::InvalidTempo)
    } else { Ok(()) }
}
fn validate_resolution(resolution: u16) -> Result<(), ClockError> {
    if !(1..=960).contains(&resolution) { Err(ClockError::InvalidResolution) } else { Ok(()) }
}
fn validate_signature(n: u8, d: u8) -> Result<(), ClockError> {
    if !(1..=32).contains(&n) || ![1, 2, 4, 8, 16, 32].contains(&d) {
        Err(ClockError::InvalidSignature)
    } else { Ok(()) }
}

impl ClockService {
    pub fn new(options: ClockOptions) -> Result<Self, ClockError> {
        validate_tempo(options.tempo)?;
        validate_resolution(options.resolution)?;
        validate_signature(options.numerator, options.denominator)?;
        if options.max_catch_up_ticks == 0 { return Err(ClockError::InvalidCatchUpLimit); }
        Ok(Self { options, state: ClockRunState::Stopped, anchor_time: 0.0,
            anchor_beat: 0.0, elapsed_at_anchor: 0.0, last_tick: 0, disposed: false })
    }

    fn assert_active(&self) -> Result<(), ClockError> {
        if self.disposed { Err(ClockError::Disposed) } else { Ok(()) }
    }

    pub fn snapshot(&self, now: f64) -> Result<ClockPosition, ClockError> {
        validate_time(now)?;
        let delta = if self.state == ClockRunState::Running {
            (now - self.anchor_time).max(0.0)
        } else { 0.0 };
        let elapsed_ms = self.elapsed_at_anchor + delta;
        let beat = (self.anchor_beat + delta * self.options.tempo / 60000.0).max(0.0);
        let beats_per_bar = f64::from(self.options.numerator) *
            (4.0 / f64::from(self.options.denominator));
        let ticks = (beat * f64::from(self.options.resolution) + 1e-9).floor();
        let bar = (beat / beats_per_bar).floor() + 1.0;
        if !beat.is_finite() || !elapsed_ms.is_finite() || ticks >= u64::MAX as f64 ||
            bar >= u64::MAX as f64 { return Err(ClockError::InvalidTime); }
        let tick = ticks as u64;
        Ok(ClockPosition {
            state: self.state, tempo: self.options.tempo, resolution: self.options.resolution,
            beat, bar: bar as u64, beat_in_bar: beat % beats_per_bar + 1.0,
            tick, subdivision: (tick % u64::from(self.options.resolution)) as u16,
            elapsed_ms, time_signature: (self.options.numerator, self.options.denominator),
        })
    }

    pub fn start(&mut self, now: f64) -> Result<(), ClockError> {
        self.assert_active()?;
        validate_time(now)?;
        if self.state == ClockRunState::Running { return Ok(()); }
        self.anchor_time = now;
        self.state = ClockRunState::Running;
        self.last_tick = (self.anchor_beat * f64::from(self.options.resolution) + 1e-9).floor() as u64;
        Ok(())
    }

    pub fn poll(&mut self, now: f64) -> Result<Vec<ClockTick>, ClockError> {
        self.assert_active()?;
        let pos = self.snapshot(now)?;
        if self.state != ClockRunState::Running || pos.tick <= self.last_tick {
            return Ok(Vec::new());
        }
        let due = pos.tick - self.last_tick;
        let count = if self.options.missed_tick_policy == MissedTickPolicy::Skip {
            1
        } else { due.min(u64::from(self.options.max_catch_up_ticks)) };
        let first = if self.options.missed_tick_policy == MissedTickPolicy::Skip {
            pos.tick
        } else { (pos.tick - count + 1).max(self.last_tick + 1) };
        let mut events = Vec::with_capacity(count as usize);
        for tick in first..=pos.tick {
            let mut p = pos;
            p.tick = tick;
            p.subdivision = (tick % u64::from(self.options.resolution)) as u16;
            events.push(ClockTick { position: p,
                scheduled_beat: tick as f64 / f64::from(self.options.resolution),
                emitted_at: now,
                missed_ticks: if tick == pos.tick { due.saturating_sub(count) } else { 0 } });
        }
        self.last_tick = pos.tick;
        Ok(events)
    }

    fn capture_running_position(&mut self, now: f64) {
        let delta = (now - self.anchor_time).max(0.0);
        self.anchor_beat += delta * self.options.tempo / 60000.0;
        self.elapsed_at_anchor += delta;
        self.anchor_time = now;
    }

    pub fn pause(&mut self, now: f64) -> Result<Vec<ClockTick>, ClockError> {
        self.assert_active()?;
        if self.state != ClockRunState::Running { validate_time(now)?; return Ok(Vec::new()); }
        let events = self.poll(now)?;
        self.capture_running_position(now);
        self.state = ClockRunState::Paused;
        Ok(events)
    }

    pub fn stop(&mut self, now: f64) -> Result<Vec<ClockTick>, ClockError> {
        self.assert_active()?;
        validate_time(now)?;
        let events = if self.state == ClockRunState::Running {
            let ticks = self.poll(now)?;
            self.capture_running_position(now);
            ticks
        } else { Vec::new() };
        self.state = ClockRunState::Stopped;
        Ok(events)
    }

    pub fn resume(&mut self, now: f64) -> Result<(), ClockError> {
        self.assert_active()?;
        if self.state == ClockRunState::Paused { self.start(now) } else {
            validate_time(now)
        }
    }

    pub fn reset(&mut self) -> Result<(), ClockError> {
        self.assert_active()?;
        self.state = ClockRunState::Stopped;
        self.anchor_beat = 0.0;
        self.elapsed_at_anchor = 0.0;
        self.last_tick = 0;
        Ok(())
    }

    pub fn set_tempo(&mut self, tempo: f64, now: f64) -> Result<Vec<ClockTick>, ClockError> {
        self.assert_active()?;
        validate_tempo(tempo)?;
        validate_time(now)?;
        if tempo == self.options.tempo { return Ok(Vec::new()); }
        let ticks = if self.state == ClockRunState::Running {
            let events = self.poll(now)?;
            self.capture_running_position(now);
            events
        } else { Vec::new() };
        self.options.tempo = tempo;
        Ok(ticks)
    }

    pub fn set_resolution(&mut self, resolution: u16, now: f64) -> Result<(), ClockError> {
        self.assert_active()?;
        validate_resolution(resolution)?;
        let beat = self.snapshot(now)?.beat;
        self.options.resolution = resolution;
        self.last_tick = (beat * f64::from(resolution) + 1e-9).floor() as u64;
        Ok(())
    }

    pub fn set_time_signature(&mut self, numerator: u8, denominator: u8) -> Result<(), ClockError> {
        self.assert_active()?;
        validate_signature(numerator, denominator)?;
        self.options.numerator = numerator;
        self.options.denominator = denominator;
        Ok(())
    }

    pub fn dispose(&mut self) {
        self.disposed = true;
        self.state = ClockRunState::Stopped;
    }
}
