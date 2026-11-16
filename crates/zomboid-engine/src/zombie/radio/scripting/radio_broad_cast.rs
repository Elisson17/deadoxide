//! zombie.radio.scripting.RadioBroadCast: the lines of one broadcast and the line counter of its airing. A broadcast
//! can name two others as its pre and post segment (an advert shared by many broadcasts), so every broadcast lives in a
//! `Broadcasts` arena and refers to those by index; the counters of a shared advert are shared as in the original.
use super::radio_line::RadioLine;

/// Index of a broadcast in its `Broadcasts` arena.
pub type BroadcastId = usize;

/// zombie.radio.scripting.RadioBroadCast
#[derive(Clone, Debug, PartialEq)]
pub struct RadioBroadCast {
    pub lines: Vec<RadioLine>,
    pub id: String,
    pub start_stamp: i32,
    pub end_stamp: i32,
    line_count: i32,
    pub pre_segment: Option<BroadcastId>,
    pub post_segment: Option<BroadcastId>,
    has_done_pre_segment: bool,
    has_done_post_pause: bool,
}

impl RadioBroadCast {
    /// `RadioBroadCast(id, startstamp, endstamp)`
    pub fn new(id: &str, start_stamp: i32, end_stamp: i32) -> Self {
        RadioBroadCast {
            lines: Vec::new(),
            id: id.to_string(),
            start_stamp,
            end_stamp,
            line_count: 0,
            pre_segment: None,
            post_segment: None,
            has_done_pre_segment: false,
            has_done_post_pause: false,
        }
    }

    /// `AddRadioLine`
    pub fn add_radio_line(&mut self, line: RadioLine) {
        self.lines.push(line);
    }

    /// `getCurrentLineNumber`
    pub fn current_line_number(&self) -> i32 {
        self.line_count
    }

    /// `setCurrentLineNumber`: a negative number is 0.
    pub fn set_current_line_number(&mut self, n: i32) {
        self.line_count = n;
        if self.line_count < 0 {
            self.line_count = 0;
        }
    }

    /// `PeekNextLineText`
    pub fn peek_next_line_text(&self) -> String {
        match usize::try_from(self.line_count)
            .ok()
            .and_then(|i| self.lines.get(i))
        {
            Some(l) => l.text.clone(),
            None => "None".to_string(),
        }
    }

    /// `getCurrentLine`
    pub fn current_line(&self) -> Option<&RadioLine> {
        usize::try_from(self.line_count)
            .ok()
            .and_then(|i| self.lines.get(i))
    }

    /// `hasDonePreSegment`, `hasDonePostPause` (private in the original; the oracle prints them).
    pub fn segment_flags(&self) -> (bool, bool) {
        (self.has_done_pre_segment, self.has_done_post_pause)
    }
}

/// The static `pauseLine` of RadioBroadCast: "~" in grey.
fn pause_line() -> RadioLine {
    RadioLine::new(Some("~"), 0.5, 0.5, 0.5, None)
}

/// Every broadcast the loaded radio data made (the original keeps them alive through the scripts and the segment links).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Broadcasts {
    pub items: Vec<RadioBroadCast>,
}

impl Broadcasts {
    pub fn add(&mut self, b: RadioBroadCast) -> BroadcastId {
        self.items.push(b);
        self.items.len() - 1
    }

    pub fn get(&self, id: BroadcastId) -> &RadioBroadCast {
        &self.items[id]
    }

    pub fn get_mut(&mut self, id: BroadcastId) -> &mut RadioBroadCast {
        &mut self.items[id]
    }

    /// `resetLineCounter()` / `resetLineCounter(doChildren)`: the counter back to 0, and (with `do_children`) the
    /// counters of the pre and post segments (which do not reset theirs further).
    pub fn reset_line_counter(&mut self, id: BroadcastId, do_children: bool) {
        self.items[id].line_count = 0;
        if do_children {
            if let Some(p) = self.items[id].pre_segment {
                self.reset_line_counter(p, false);
            }
            if let Some(p) = self.items[id].post_segment {
                self.reset_line_counter(p, false);
            }
        }
    }

    /// `getNextLine(doChildren)`: the pre segment's lines first (then a pause line), the broadcast's own lines, then
    /// (after a pause line) the post segment's.
    pub fn get_next_line(&mut self, id: BroadcastId, do_children: bool) -> Option<RadioLine> {
        let (pre, post) = (self.items[id].pre_segment, self.items[id].post_segment);
        if do_children
            && !self.items[id].has_done_pre_segment
            && self.items[id].line_count == 0
            && pre.is_some()
        {
            let r = self.get_next_line(pre?, true);
            if r.is_some() {
                return r;
            }
            self.items[id].has_done_pre_segment = true;
            return Some(pause_line());
        }
        let b = &self.items[id];
        let r = usize::try_from(b.line_count)
            .ok()
            .and_then(|i| b.lines.get(i))
            .cloned();
        if !do_children || r.is_some() || post.is_none() {
            self.items[id].line_count += 1;
            r
        } else if !self.items[id].has_done_post_pause {
            self.items[id].has_done_post_pause = true;
            Some(pause_line())
        } else {
            self.get_next_line(post?, true)
        }
    }
}
