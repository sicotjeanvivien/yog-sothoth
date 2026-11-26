use chrono::TimeDelta;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Window {
    /// The last hour, through the last tranche written: never a frozen period.
    Hour,
    Day,
    Week,
    Month,
}

impl Window {
    /// The windows frozen periods are written for; [`Window::Hour`] is read from the tranches.
    pub const ALL: [Window; 3] = [Window::Day, Window::Week, Window::Month];

    pub const fn hours(self) -> i32 {
        match self {
            Window::Hour => 1,
            Window::Day => 24,
            Window::Week => 168,
            Window::Month => 720,
        }
    }

    pub fn duration(self) -> TimeDelta {
        TimeDelta::hours(self.hours().into())
    }

    /// ⚠️ Also the `period` CHECKs of migrations 029 and 034: a new label fails their inserts.
    pub const fn as_str(self) -> &'static str {
        match self {
            Window::Hour => "1h",
            Window::Day => "24h",
            Window::Week => "7d",
            Window::Month => "30d",
        }
    }
}

#[cfg(test)]
#[path = "window_tests.rs"]
mod tests;
