//! Timing curves and tweens (Core Animation's ease-in-ease-out and friends).

/// Cubic Bézier timing function with control points (x1,y1) (x2,y2), like CAMediaTimingFunction.
pub fn bezier(x1: f32, y1: f32, x2: f32, y2: f32, t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    let sample = |a: f32, b: f32, s: f32| {
        let u = 1.0 - s;
        3.0 * u * u * s * a + 3.0 * u * s * s * b + s * s * s
    };
    let (mut lo, mut hi) = (0.0f32, 1.0f32);
    let mut s = t;
    for _ in 0..24 {
        let x = sample(x1, x2, s);
        if (x - t).abs() < 1e-5 {
            break;
        }
        if x < t {
            lo = s;
        } else {
            hi = s;
        }
        s = (lo + hi) * 0.5;
    }
    sample(y1, y2, s)
}

/// CAMediaTimingFunction(name: .easeInEaseOut)
pub fn ease_in_out(t: f32) -> f32 {
    bezier(0.42, 0.0, 0.58, 1.0, t)
}

#[derive(Clone, Copy, Debug)]
pub enum Curve {
    EaseInOut,
    Linear,
}

impl Curve {
    pub fn at(self, t: f32) -> f32 {
        match self {
            Curve::EaseInOut => ease_in_out(t),
            Curve::Linear => t.clamp(0.0, 1.0),
        }
    }
}

/// A value that glides from where it is to a target.
#[derive(Clone, Copy, Debug)]
pub struct Tween {
    from: f32,
    to: f32,
    start: f64,
    dur: f32,
    curve: Curve,
}

impl Tween {
    pub fn new(v: f32) -> Self {
        Tween { from: v, to: v, start: 0.0, dur: 0.0, curve: Curve::EaseInOut }
    }

    pub fn value(&self, now: f64) -> f32 {
        if self.dur <= 0.0 {
            return self.to;
        }
        let t = ((now - self.start) as f32 / self.dur).clamp(0.0, 1.0);
        self.from + (self.to - self.from) * self.curve.at(t)
    }

    pub fn target(&self) -> f32 {
        self.to
    }

    pub fn is_done(&self, now: f64) -> bool {
        self.dur <= 0.0 || (now - self.start) as f32 >= self.dur
    }

    pub fn go(&mut self, now: f64, to: f32, dur: f32) {
        self.go_with(now, to, dur, Curve::EaseInOut);
    }

    pub fn go_with(&mut self, now: f64, to: f32, dur: f32, curve: Curve) {
        self.from = self.value(now);
        self.to = to;
        self.start = now;
        self.dur = dur;
        self.curve = curve;
    }

    pub fn set(&mut self, v: f32) {
        self.from = v;
        self.to = v;
        self.dur = 0.0;
    }
}

pub fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

impl Tween {
    /// Like `go`, but the motion begins `delay` seconds from now.
    pub fn go_after(&mut self, now: f64, delay: f32, to: f32, dur: f32) {
        let from = self.value(now);
        self.from = from;
        self.to = to;
        self.start = now + delay as f64;
        self.dur = dur;
        self.curve = Curve::EaseInOut;
    }
}
