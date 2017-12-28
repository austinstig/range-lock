//! # Utils
//! Contains a simple implementation of an interval tree.

use std::collections::BTreeSet;
use std::ops::{Range, RangeTo, RangeFrom};
use std::cmp::{min, max, PartialEq, Ordering};
use num::PrimInt;

/// # Interval
/// An interval represent the a continuous segment of a number line.
/// It handles segments of integer number lines because these are Ranges used for indexing a Vec.
#[derive(Debug, Ord, Eq)]
pub struct Interval<T: PrimInt>(T, T);

impl<T> Interval<T>
where
    T: PrimInt,
{
    /// get the midpoint of the interval for sorting
    fn center(&self) -> T {
        let a = self.0;
        let b = self.1;
        (a - b) / (T::one() + T::one())
    }

    /// determine if this interval overlaps with another interval
    fn overlaps(&self, other: &Self) -> bool {
        self.0 <= other.1 && other.0 <= self.1
    }
}

impl<T> From<Range<T>> for Interval<T>
where
    T: PrimInt,
{
    /// convert from a Range into an Interval
    fn from(r: Range<T>) -> Interval<T> {
        Interval(min(r.start, r.end), max(r.start, r.end))
    }
}

impl<T> From<RangeTo<T>> for Interval<T>
where
    T: PrimInt,
{
    /// convert from a RangeTo into an Interval
    fn from(r: RangeTo<T>) -> Interval<T> {
        Interval(T::min_value(), r.end)
    }
}

impl<T> From<RangeFrom<T>> for Interval<T>
where
    T: PrimInt,
{
    /// convert from a RangeFrom into an Interval
    fn from(r: RangeFrom<T>) -> Interval<T> {
        Interval(r.start, T::max_value())
    }
}

impl<T> PartialEq for Interval<T>
where
    T: PrimInt,
{
    /// determine when two intervals are equal
    fn eq(&self, other: &Self) -> bool {
        self.center() == other.center()
    }
}


impl<T> PartialOrd for Interval<T>
where
    T: PrimInt,
{
    /// determine how intervals should be ordered
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        if self.center() == other.center() {
            Some(Ordering::Equal)
        } else if self.center() < other.center() {
            Some(Ordering::Less)
        } else if self.center() > other.center() {
            Some(Ordering::Greater)
        } else {
            None
        }
    }
}

/// # IntervalTree
/// A binary tree of intervals. This allows for fast interval overlap lookups.
/// It is backed by a BTreeSet.
#[derive(Default, Debug)]
pub struct IntervalTree<T: PrimInt>(BTreeSet<Interval<T>>);

impl<T> IntervalTree<T>
where
    T: PrimInt,
{
    fn tree_mut(&mut self) -> &mut BTreeSet<Interval<T>> {
        &mut self.0
    }
    fn tree(&self) -> &BTreeSet<Interval<T>> {
        &self.0
    }
    #[allow(dead_code)]
    pub fn extend<J: Into<Interval<T>>, I: IntoIterator<Item = J>>(&mut self, intervals: I) {
        self.tree_mut().extend(intervals.into_iter().map(J::into));
    }
    pub fn insert<I: Into<Interval<T>>>(&mut self, interval: I) {
        self.tree_mut().insert(I::into(interval));
    }
    pub fn overlap_search<I: Into<Interval<T>>>(&self, interval: I) -> bool {
        let interval = I::into(interval);
        self.tree().iter().any(|ref e| e.overlaps(&interval))
    }
    pub fn remove<I: Into<Interval<T>>>(&mut self, interval: I) {
        let interval = I::into(interval);
        self.tree_mut().remove(&interval);
    }
}

// aliases for different types
pub type ReadIntervalTree = IntervalTree<usize>;
pub type WriteIntervalTree = IntervalTree<usize>;

/// # IntervalForest
/// Stores a read and write interval tree
#[derive(Default)]
pub struct IntervalForest(ReadIntervalTree, WriteIntervalTree);

impl IntervalForest {
    /// get immutable access to the read tree
    fn read_tree(&self) -> &ReadIntervalTree {
        &self.0
    }

    /// get mutable access to the read tree
    fn read_tree_mut(&mut self) -> &mut ReadIntervalTree {
        &mut self.0
    }

    /// get immutable access to the write tree
    fn write_tree(&self) -> &WriteIntervalTree {
        &self.1
    }

    /// get mutable access to the write tree
    fn write_tree_mut(&mut self) -> &mut WriteIntervalTree {
        &mut self.1
    }

    /// add an interval to the interval forest
    pub fn insert(&mut self, rtype: RangeType) {
        match rtype {
            RangeType::Read(r1) => self.read_tree_mut().insert(r1),
            RangeType::Write(r2) => self.write_tree_mut().insert(r2),
        }
    }

    /// remove an interval from the interval forest
    pub fn delete(&mut self, rtype: RangeType) {
        match rtype {
            RangeType::Read(r1) => self.read_tree_mut().remove(r1),
            RangeType::Write(r2) => self.write_tree_mut().remove(r2),
        }
    }

    /// return true if range type conflict with a stored interval
    /// a conflict occurs if a read input overlaps with an existing write,
    /// or if a write input overlaps with an existing read or write
    pub fn conflicts_with(&self, rtype: &RangeType) -> bool {
        match rtype {
            &RangeType::Read(ref r1) => self.write_tree().overlap_search(r1.clone()),
            &RangeType::Write(ref r2) => {
                self.write_tree().overlap_search(r2.clone()) ||
                    self.read_tree().overlap_search(r2.clone())
            }
        }
    }
}

/// # RangeType
/// The type of range.
#[derive(is_enum_variant)]
#[derive(PartialEq, Clone, Debug)]
pub enum RangeType {
    Read(Range<usize>),
    Write(Range<usize>),
}


#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn lower_overlaps_test() {
        let mut tree: IntervalTree<usize> = IntervalTree::default();
        tree.insert(1..10);
        assert!(tree.overlap_search(0..3))
    }
    #[test]
    fn upper_overlaps_test() {
        let mut tree: IntervalTree<usize> = IntervalTree::default();
        tree.insert(0..10);
        assert!(tree.overlap_search(9..12))
    }
    #[test]
    fn center_overlaps_test() {
        let mut tree: IntervalTree<usize> = IntervalTree::default();
        tree.insert(0..10);
        assert!(tree.overlap_search(4..6))
    }
    #[test]
    fn no_overlaps_test() {
        let mut tree: IntervalTree<usize> = IntervalTree::default();
        tree.insert(0..10);
        assert!(!tree.overlap_search(11..30))
    }
    #[test]
    fn remove_no_overlaps_test() {
        let mut tree: IntervalTree<usize> = IntervalTree::default();
        tree.insert(0..10);
        tree.remove(0..10);
        assert!(!tree.overlap_search(4..6))
    }
    #[test]
    fn extend_overlap_test() {
        let mut tree: IntervalTree<usize> = IntervalTree::default();
        tree.extend(vec![0..10,11..20]);
        assert!(tree.overlap_search(4..6))
    }
}