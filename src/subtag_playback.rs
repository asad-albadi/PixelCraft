use pixelcraft::model::FrameTag;

use std::{cmp::Reverse, collections::BTreeMap};

struct Range {
    from: i32,
    to: i32,
    direction: u8,
    repeats: u16,
    children: [Vec<usize>; 2],
}

impl Range {
    fn new(from: usize, to: usize, direction: u8, repeats: u16) -> Self {
        Self {
            from: from as i32,
            to: to as i32,
            direction,
            repeats: if repeats == 0 {
                if direction >= 2 { 2 } else { 1 }
            } else {
                repeats
            },
            children: Default::default(),
        }
    }

    fn first(&self, reverse: bool) -> i32 {
        if reverse { self.to } else { self.from }
    }
}

struct Cursor {
    range: usize,
    reverse: bool,
    remaining: u16,
    frame: i32,
    next_child: usize,
}

impl Cursor {
    fn enter(range: usize, node: &Range, parent_reverse: bool) -> Self {
        let reverse = parent_reverse ^ matches!(node.direction, 1 | 3);
        Self {
            range,
            reverse,
            remaining: node.repeats,
            frame: node.first(reverse),
            next_child: 0,
        }
    }
}

/// Streams one pass through the playback scope. The stack holds only active
/// ranges, so repeat counts do not multiply the amount of memory allocated.
pub(super) struct SubtagFrames {
    ranges: Vec<Range>,
    stack: Vec<Cursor>,
    last: Option<usize>,
    skip_turn_duplicate: bool,
}

impl SubtagFrames {
    pub(super) fn new(count: usize, tags: &[FrameTag], active: Option<usize>) -> Self {
        let valid = |tag: &FrameTag| tag.from <= tag.to && tag.to < count;
        let active_tag = active
            .and_then(|index| tags.get(index))
            .filter(|tag| valid(tag));
        let root = active_tag.map_or_else(
            || Range::new(0, count.saturating_sub(1), 0, 1),
            |tag| Range::new(tag.from, tag.to, tag.direction, 0),
        );
        let mut order: Vec<_> = tags
            .iter()
            .enumerate()
            .filter(|&(index, tag)| {
                valid(tag)
                    && active_tag.is_none_or(|outer| {
                        active != Some(index)
                            && outer.from <= tag.from
                            && tag.to <= outer.to
                            && ((outer.from, outer.to) != (tag.from, tag.to)
                                || active.is_some_and(|root| root < index))
                    })
            })
            .collect();
        order.sort_unstable_by_key(|&(index, tag)| (tag.from, Reverse(tag.to), index));

        let mut ranges = Vec::with_capacity(order.len() + 1);
        ranges.push(root);
        // With starts processed in ascending order, the smallest qualifying
        // end selects a containing range. A later range wins ties on the end.
        let mut by_end = BTreeMap::new();
        for (_, tag) in order {
            let parent = by_end.range(tag.to..).next().map_or(0, |(_, &index)| index);
            let index = ranges.len();
            ranges[parent].children[0].push(index);
            ranges.push(Range::new(tag.from, tag.to, tag.direction, tag.repeat));
            by_end.insert(tag.to, index);
        }
        for index in 0..ranges.len() {
            let mut children = ranges[index].children[0].clone();
            children.sort_unstable_by_key(|&child| (Reverse(ranges[child].to), child));
            ranges[index].children[1] = children;
        }

        let mut playback = Self {
            ranges,
            stack: Vec::new(),
            last: None,
            skip_turn_duplicate: false,
        };
        if count > 0 {
            playback.rewind();
        }
        playback
    }

    pub(super) fn rewind(&mut self) {
        self.stack.clear();
        self.stack.push(Cursor::enter(0, &self.ranges[0], false));
        self.last = None;
        self.skip_turn_duplicate = false;
    }
}

impl Iterator for SubtagFrames {
    type Item = usize;

    fn next(&mut self) -> Option<Self::Item> {
        while let Some(cursor) = self.stack.last_mut() {
            let node = &self.ranges[cursor.range];
            let reverse = cursor.reverse;
            let child = node.children[usize::from(reverse)]
                .get(cursor.next_child)
                .copied();
            if let Some(child) = child {
                let child_range = &self.ranges[child];
                let at_child = if reverse {
                    cursor.frame <= child_range.to
                } else {
                    cursor.frame >= child_range.from
                };
                if at_child {
                    cursor.next_child += 1;
                    cursor.frame = if reverse {
                        cursor.frame.min(child_range.from - 1)
                    } else {
                        cursor.frame.max(child_range.to + 1)
                    };
                    self.stack.push(Cursor::enter(child, child_range, reverse));
                    continue;
                }
            }

            if (node.from..=node.to).contains(&cursor.frame) {
                let frame = cursor.frame as usize;
                cursor.frame += if reverse { -1 } else { 1 };
                let skip =
                    std::mem::take(&mut self.skip_turn_duplicate) && self.last == Some(frame);
                if !skip {
                    self.last = Some(frame);
                    return Some(frame);
                }
            } else {
                cursor.remaining -= 1;
                if cursor.remaining == 0 {
                    self.stack.pop();
                    continue;
                }
                if node.direction >= 2 && node.from != node.to {
                    cursor.reverse = !reverse;
                    self.skip_turn_duplicate = true;
                }
                cursor.frame = node.first(cursor.reverse);
                cursor.next_child = 0;
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tag(from: usize, to: usize, direction: u8, repeat: u16) -> FrameTag {
        FrameTag {
            editor_id: Default::default(),
            name: String::new(),
            from,
            to,
            direction,
            repeat,
            color: [0; 3],
        }
    }

    #[test]
    fn nested_repeats_and_ping_pong_turns_compose() {
        let tags = [tag(1, 5, 0, 2), tag(2, 4, 2, 3)];
        let frames: Vec<_> = SubtagFrames::new(7, &tags, None).collect();
        assert_eq!(
            frames,
            [0, 1, 2, 3, 4, 3, 2, 3, 4, 5, 1, 2, 3, 4, 3, 2, 3, 4, 5, 6]
        );
    }

    #[test]
    fn reverse_parent_flips_child_direction_and_cascade_wraps_overlapping_tags() {
        let tags = [tag(0, 4, 2, 2), tag(1, 3, 1, 3)];
        assert_eq!(
            SubtagFrames::new(5, &tags, None).collect::<Vec<_>>(),
            [
                0, 3, 2, 1, 3, 2, 1, 3, 2, 1, 4, 1, 2, 3, 1, 2, 3, 1, 2, 3, 0
            ],
        );
        let overlapping = [tag(1, 3, 0, 2), tag(2, 4, 0, 2)];
        assert_eq!(
            SubtagFrames::new(5, &overlapping, None).collect::<Vec<_>>(),
            [0, 1, 2, 3, 1, 2, 3, 2, 3, 4, 2, 3, 4],
        );
    }

    #[test]
    fn selected_root_and_large_nested_counts_remain_streamed() {
        let tags = [
            tag(0, 5, 1, u16::MAX),
            tag(1, 4, 0, u16::MAX),
            tag(2, 3, 1, u16::MAX),
        ];
        let frames = SubtagFrames::new(6, &tags, Some(0));
        assert_eq!(
            frames.take(9).collect::<Vec<_>>(),
            [5, 4, 2, 3, 2, 3, 2, 3, 2]
        );
        let finite = [tag(1, 4, 3, 9), tag(2, 3, 0, 0), tag(0, 0, 0, 8)];
        assert_eq!(
            SubtagFrames::new(5, &finite, Some(0)).collect::<Vec<_>>(),
            [4, 3, 2, 1, 2, 3, 4]
        );
    }
}
