use crate::{lisp_time, EmacsInt};
use std::cmp::Ordering;

use std::ops::{Add, Sub};

pub const LO_TIME_BITS: i32 = 16;

pub type LispTime = lisp_time;

impl LispTime {
    pub fn into_vec(self, nelem: usize) -> Vec<EmacsInt> {
        let mut v = Vec::with_capacity(nelem);

        if nelem >= 2 {
            v.push(self.hi);
            v.push(self.lo.into());
        }
        if nelem >= 3 {
            v.push(self.us.into());
        }
        if nelem > 3 {
            v.push(self.ps.into());
        }

        v
    }
}

impl PartialEq for LispTime {
    fn eq(&self, other: &LispTime) -> bool {
        self.hi == other.hi && self.lo == other.lo && self.us == other.us && self.ps == other.ps
    }
}

impl Eq for LispTime {}

impl PartialOrd for LispTime {
    fn partial_cmp(&self, other: &LispTime) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for LispTime {
    fn cmp(&self, other: &LispTime) -> Ordering {
        self.hi
            .cmp(&other.hi)
            .then_with(|| self.lo.cmp(&other.lo))
            .then_with(|| self.us.cmp(&other.us))
            .then_with(|| self.ps.cmp(&other.ps))
    }
}

#[allow(clippy::suspicious_arithmetic_impl)]
impl Add for LispTime {
    type Output = LispTime;

    fn add(self, other: LispTime) -> LispTime {
        let mut hi = self.hi + other.hi;
        let mut lo = self.lo + other.lo;
        let mut us = self.us + other.us;
        let mut ps = self.ps + other.ps;

        if ps >= 1_000_000 {
            us += 1;
            ps -= 1_000_000;
        }
        if us >= 1_000_000 {
            lo += 1;
            us -= 1_000_000;
        }
        if lo >= 1 << LO_TIME_BITS {
            hi += 1;
            lo -= 1 << LO_TIME_BITS;
        }

        LispTime { hi, lo, us, ps }
    }
}

#[allow(clippy::suspicious_arithmetic_impl)]
impl Sub for LispTime {
    type Output = LispTime;

    fn sub(self, other: LispTime) -> LispTime {
        let mut hi = self.hi - other.hi;
        let mut lo = self.lo - other.lo;
        let mut us = self.us - other.us;
        let mut ps = self.ps - other.ps;

        if ps < 0 {
            us -= 1;
            ps += 1_000_000;
        }
        if us < 0 {
            lo -= 1;
            us += 1_000_000;
        }
        if hi < 0 {
            hi -= 1;
            lo += 1 << LO_TIME_BITS;
        }

        LispTime { hi, lo, us, ps }
    }
}
