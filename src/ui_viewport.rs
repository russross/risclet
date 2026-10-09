use std::ops::Range;

use super::{MemoryPane, Tui};
use crate::riscv::SP;
use crate::trace::{Effects, FrameChange};

const FORWARD_LIMIT: usize = 1_000;
const BACKWARD_LIMIT: usize = 500;

#[derive(Clone, Copy)]
pub(super) struct MemoryViewport {
    start: i64,
    height: u16,
}

// Positions are segment-relative rows, increasing upward on the screen.
// Keeping an interval preserves every tie until a later priority resolves it.
struct Positions {
    low: i64,
    high: i64,
    height: i64,
}

impl Positions {
    fn contain(&mut self, rows: &Range<i64>) -> bool {
        let low = self.low.max(rows.end - self.height);
        let high = self.high.min(rows.start);
        if low > high {
            return false;
        }
        self.low = low;
        self.high = high;
        true
    }

    // Overlap is maximal on a plateau between these two ideal positions.
    // A wholly unreachable range contributes no preference at all.
    fn prefer(&mut self, rows: &Range<i64>) {
        if rows.end <= self.low || rows.start >= self.high + self.height {
            return;
        }
        let a = rows.start.min(rows.end - self.height);
        let b = rows.start.max(rows.end - self.height);
        let low = a.clamp(self.low, self.high);
        let high = b.clamp(self.low, self.high);
        self.low = low;
        self.high = high;
    }
}

#[derive(Clone)]
struct StackState {
    sp: u32,
    frames: Vec<u32>,
}

impl StackState {
    // Each scan direction owns its frame history; no machine state is mutated.
    fn step(&mut self, effect: &Effects, forward: bool) {
        if let Some(write) = effect.reg_write
            && write.register == SP
        {
            self.sp =
                if forward { write.new_value } else { write.old_value } as u32;
        }
        match (effect.frame_change, forward) {
            (Some(FrameChange::Enter(frame)), true)
            | (Some(FrameChange::Leave(frame)), false) => {
                self.frames.push(frame)
            }
            (Some(FrameChange::Enter(_)), false)
            | (Some(FrameChange::Leave(_)), true) => {
                self.frames.pop();
            }
            (None, _) => {}
        }
    }
}

struct Access {
    rows: Range<i64>,
    region: Range<i64>,
}

struct Segment {
    kind: MemoryPane,
    bounds: Range<u32>,
    boundaries: Vec<u32>,
}

impl Segment {
    fn rows(&self, start: u32, end: u64) -> Range<i64> {
        let base = i64::from(self.bounds.start);
        (i64::from(start) - base) / 8..(end as i64 - base + 7) / 8
    }

    fn access(
        &self,
        address: u32,
        length: usize,
        stack: &StackState,
    ) -> Option<Access> {
        if length == 0 || !self.bounds.contains(&address) {
            return None;
        }
        let end = (u64::from(address) + length as u64)
            .min(u64::from(self.bounds.end));
        let mut low = self.bounds.start;
        let mut high = self.bounds.end;
        let boundaries = if self.kind == MemoryPane::Stack {
            let mut boundaries = stack.frames.clone();
            boundaries.push(stack.sp);
            boundaries
        } else {
            self.boundaries.clone()
        };

        // Region boundaries belong to the state at this access, including
        // frames that exist only in one direction of the trace scan.
        for boundary in boundaries {
            if boundary <= address {
                low = low.max(boundary);
            }
            if u64::from(boundary) >= end {
                high = high.min(boundary);
            }
        }
        Some(Access {
            rows: self.rows(address, end),
            region: self.rows(low, u64::from(high)),
        })
    }

    fn accesses(&self, effect: &Effects, stack: &StackState) -> Vec<Access> {
        if self.kind == MemoryPane::Text {
            return self
                .access(
                    effect.instruction.address,
                    effect.instruction.length as usize,
                    stack,
                )
                .into_iter()
                .collect();
        }
        // Reads precede writes when an instruction exposes both kinds of access.
        let read = effect.mem_read.as_ref().and_then(|value| {
            self.access(value.address, value.value.len(), stack)
        });
        let write = effect.mem_write.as_ref().and_then(|value| {
            self.access(value.address, value.new_value.len(), stack)
        });
        read.into_iter().chain(write).collect()
    }

    // Highlight selection follows the machine's read-before-write convention,
    // while lookahead can still gather both accesses from one instruction.
    fn focus(&self, effect: &Effects, stack: &StackState) -> Option<Access> {
        if self.kind == MemoryPane::Text {
            self.access(
                effect.instruction.address,
                effect.instruction.length as usize,
                stack,
            )
        } else if let Some(read) = &effect.mem_read {
            self.access(read.address, read.value.len(), stack)
        } else {
            effect.mem_write.as_ref().and_then(|write| {
                self.access(write.address, write.new_value.len(), stack)
            })
        }
    }
}

impl Tui {
    pub(super) fn memory_viewport(
        &mut self,
        kind: MemoryPane,
        height: u16,
    ) -> i64 {
        let (bounds, colors, focus) = match kind {
            MemoryPane::Stack => (
                self.machine.stack_start()..self.machine.stack_end(),
                &self.data_colors,
                self.machine.most_recent_stack(),
            ),
            MemoryPane::Data => (
                self.machine.data_start()..self.machine.data_end(),
                &self.data_colors,
                self.machine.most_recent_data(),
            ),
            MemoryPane::Text => {
                let instruction =
                    &self.sequence[self.sequence_index].instruction;
                (
                    self.machine.text_start()..self.machine.text_end(),
                    &self.text_colors,
                    (instruction.address, instruction.length as usize),
                )
            }
        };
        let segment = Segment {
            kind,
            bounds,
            boundaries: colors.iter().map(|&(address, _)| address).collect(),
        };
        let rows =
            i64::from((segment.bounds.end - segment.bounds.start).div_ceil(8));
        let h = i64::from(height);
        if h == 0 {
            return 0;
        }

        // Short segments never move; their empty rows follow segment alignment.
        let start = if rows <= h {
            match kind {
                MemoryPane::Stack => rows - h,
                MemoryPane::Data => -(h - rows) / 2,
                MemoryPane::Text => 0,
            }
        } else {
            let highlight =
                segment.rows(focus.0, u64::from(focus.0) + focus.1 as u64);
            if highlight.end - highlight.start >= h {
                highlight.start
            } else if let Some(previous) = self.memory_viewports[kind as usize]
                && previous.height == height
                && (focus.1 == 0
                    || (previous.start <= highlight.start
                        && highlight.end <= previous.start + h))
            {
                previous.start
            } else {
                self.plan_viewport(&segment, h, rows)
            }
        };
        self.memory_viewports[kind as usize] =
            Some(MemoryViewport { start, height });
        start
    }

    fn plan_viewport(
        &mut self,
        segment: &Segment,
        height: i64,
        rows: i64,
    ) -> i64 {
        let initial = StackState {
            sp: self.machine.get(SP) as u32,
            frames: self.machine.stack_frames().to_vec(),
        };
        let mut positions = Positions { low: 0, high: rows - height, height };
        let mut regions = Vec::new();
        let mut terminals = Vec::new();
        let mut extent: Option<Range<i64>> = None;

        // Seed with the same current-or-most-recent access used for highlighting.
        // Rewind frame metadata to that access rather than assigning today's frame.
        let mut past = initial.clone();
        for index in (0..=self.sequence_index).rev() {
            if index < self.sequence_index {
                past.step(&self.sequence[index], false);
            }
            if let Some(access) = segment.focus(&self.sequence[index], &past) {
                if access.rows.end - access.rows.start >= height {
                    return access.rows.start;
                }
                positions.contain(&access.rows);
                extent = Some(access.rows);
                regions.push(access.region);
                break;
            }
        }

        // Scan instructions (not accesses) at a two-forward/one-backward ratio.
        // Overflow stops only its own direction; exact fits settle placement.
        let mut forward = initial.clone();
        let mut backward = initial;
        let mut stopped = [false; 2];
        let rounds = FORWARD_LIMIT.div_ceil(2).max(BACKWARD_LIMIT);
        for turn in 0..=rounds * 3 {
            let slot = turn.saturating_sub(1);
            let back = turn > 0 && slot % 3 == 2;
            let offset = if turn == 0 {
                0
            } else if back {
                slot / 3 + 1
            } else {
                slot - slot / 3 + 1
            };
            let direction = usize::from(back);
            let limit = if back { BACKWARD_LIMIT } else { FORWARD_LIMIT };
            if stopped[direction] || offset > limit {
                continue;
            }
            let index = if back {
                self.sequence_index.checked_sub(offset)
            } else {
                self.sequence_index
                    .checked_add(offset)
                    .filter(|&i| i < self.sequence.len())
            };
            let Some(index) = index else {
                stopped[direction] = true;
                continue;
            };
            let state = if back {
                backward.step(&self.sequence[index], false);
                &backward
            } else {
                if offset > 0 {
                    forward.step(&self.sequence[index - 1], true);
                }
                &forward
            };
            for access in segment.accesses(&self.sequence[index], state) {
                if extent.is_none()
                    && access.rows.end - access.rows.start >= height
                {
                    return access.rows.start;
                }
                if !positions.contain(&access.rows) {
                    terminals.push(access.rows);
                    regions.push(access.region);
                    stopped[direction] = true;
                    break;
                }
                extent = Some(match extent {
                    Some(old) => {
                        old.start.min(access.rows.start)
                            ..old.end.max(access.rows.end)
                    }
                    None => access.rows.clone(),
                });
                regions.push(access.region);
                if positions.low == positions.high {
                    return positions.low;
                }
            }
        }

        // Values outrank regions. Each preference narrows only the remaining
        // placement freedom, preserving all earlier maximum-overlap decisions.
        for terminal in terminals {
            positions.prefer(&terminal);
        }
        for region in regions {
            positions.prefer(&region);
            // Complete regions also determine the center of the gathered set.
            // Partial regions have already consumed their placement preference.
            if positions.low >= region.end - height
                && positions.high <= region.start
            {
                extent = Some(match extent {
                    Some(old) => {
                        old.start.min(region.start)..old.end.max(region.end)
                    }
                    None => region,
                });
            }
        }
        let Some(extent) = extent else {
            return if segment.kind == MemoryPane::Stack {
                rows - height
            } else {
                0
            };
        };
        let preferred = match segment.kind {
            MemoryPane::Stack => positions.high,
            MemoryPane::Data | MemoryPane::Text => {
                (extent.start + extent.end - height + 1).div_euclid(2)
            }
        };
        preferred.clamp(positions.low, positions.high)
    }
}
