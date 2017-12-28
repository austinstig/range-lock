//! # Lock
//! The implementation of a range rw lock that is evaluated dynamically at runtime.

/// imports from super
use std::fmt::Debug;
use std::sync::Mutex;
use std::sync::MutexGuard;
use std::sync::PoisonError;
use std::cell::UnsafeCell;
use std::ops::Range;
use super::utils::RangeType;
use super::utils::IntervalForest;

/// An IntervalForest behind a mutex
pub type GuardedRanges<'a> = MutexGuard<'a, IntervalForest>;

/// # LockRange
/// A lock over a range of value [T]. Drop the range from the lock
/// when the LockedRange goes out of scope.
#[derive(new)]
pub struct LockRange<'lock, T: 'lock + Copy + Debug> {
    rlock: &'lock RangeLock<T>,
    rtype: RangeType,
}

impl<'lock, T> Drop for LockRange<'lock, T>
where
    T: 'lock + Copy + Debug,
{
    fn drop(&mut self) {
        self.rlock.remove(&self.rtype);
    }
}


/// # LockGuard
/// A read or write guard
#[derive(Debug)]
pub enum LockGuard<'lock, T: 'lock + Copy + Debug> {
    Read(&'lock [T]),
    Write(&'lock mut [T]),
}

impl<'lock, T> From<&'lock [T]> for LockGuard<'lock, T>
where
    T: 'lock + Copy + Debug,
{
    /// convert from a borrow of a slice of [T]
    fn from(s: &'lock [T]) -> LockGuard<'lock, T> {
        LockGuard::Read(s)
    }
}

impl<'lock, T> From<&'lock mut [T]> for LockGuard<'lock, T>
where
    T: 'lock + Copy + Debug,
{
    /// convert from a mutable borrow of a slice of [T]
    fn from(s: &'lock mut [T]) -> LockGuard<'lock, T> {
        LockGuard::Write(s)
    }
}

#[derive(new)]
pub struct RangeLockGuard<'lock, T: 'lock + Copy + Debug>(
    pub LockGuard<'lock, T>,
    pub LockRange<'lock, T>
);

/// # RangeLockResult
/// The result of a range locking attempt
pub enum RangeLockResult<'lock, T: 'lock + Copy + Debug> {
    Ok(RangeLockGuard<'lock, T>),
    RangeConflict,
    BadRange,
    OtherError,
}

/// # RangeLock
/// Allows multiple immutable and mutable borrows based on access ranges.
pub struct RangeLock<T: Copy + Debug> {
    ranges: Mutex<IntervalForest>,
    data: UnsafeCell<Vec<T>>,
}

impl<T> RangeLock<T> where T: Copy + Debug {
    fn len(&self) -> u64 {
        unsafe { (*self.data.get()).len() as u64 }
    }
}

/// make sure the RangeLock can be shared between threads
unsafe impl<T> Sync for RangeLock<T>
where
    T: Copy + Debug,
{
}

/// a conversion from a vec of T data
impl<T> From<Vec<T>> for RangeLock<T>
where
    T: Copy + Debug,
{
    fn from(v: Vec<T>) -> RangeLock<T> {
        RangeLock {
            ranges: Mutex::new(IntervalForest::default()),
            data: UnsafeCell::new(v),
        }
    }
}


impl<T> RangeLock<T>
where
    T: Copy + Debug,
{
    /// get a reference to the data
    fn data(&self) -> Option<&[T]> {
        unsafe { self.data.get().as_ref().map(Vec::as_slice) }
    }

    /// get a mutable reference to the data
    fn data_mut(&self) -> Option<&mut [T]> {
        unsafe { self.data.get().as_mut().map(Vec::as_mut_slice) }
    }

    /// return a lock of the ranges
    fn lock(&self) -> Result<GuardedRanges, PoisonError<GuardedRanges>> {
        self.ranges.lock()
    }

    /// get a range of data to mutate
    pub fn get_mut(&self, range: Range<usize>) -> RangeLockResult<T> {
        // check range limits
        if !(range.end < (self.len() as usize)) {
            return RangeLockResult::BadRange;
        }
        // get a typed range
        let trange = RangeType::Write(range.clone());
        // evaluate whether or not the range can be accessed
        if let Ok(mut ranges) = self.lock() {
            // check for any conflicts with existing reads and writes
            if ranges.conflicts_with(&trange) {
                return RangeLockResult::RangeConflict;
            }
            // add the range to the set of ranges
            ranges.insert(trange.clone());
        }
        // return the range of data for mutation
        match self.data_mut() {
            Some(values) => RangeLockResult::Ok(RangeLockGuard::new(
                LockGuard::from(&mut values[range]),
                LockRange::new(&self, trange),
            )),
            None => RangeLockResult::OtherError,
        }
    }

    /// get a range of data to read
    pub fn get(&self, range: Range<usize>) -> RangeLockResult<T> {
        // check range limits
        if !(range.end < (self.len() as usize)) {
            return RangeLockResult::BadRange;
        }
        // get a typed range
        let trange = RangeType::Read(range.clone());
        // evaluate whether or not the range can be accessed
        if let Ok(mut ranges) = self.lock() {
            // check for any conflicts with existing writes
            if ranges.conflicts_with(&trange) {
                return RangeLockResult::RangeConflict;
            }
            // add the range to the set of ranges
            ranges.insert(trange.clone());
        }
        // return the range of data for mutation
        match self.data() {
            Some(values) => RangeLockResult::Ok(RangeLockGuard::new(
                LockGuard::from(&values[range]),
                LockRange::new(&self, trange),
            )),
            None => RangeLockResult::OtherError,
        }
    }

    /// remove a range
    fn remove(&self, range: &RangeType) {
        if let Ok(mut ranges) = self.ranges.lock() {
            ranges.delete(range.clone());
            println!("remove({:?})", range);
        }
    }
}



#[cfg(test)]
mod tests {

    use super::*;
    use std::sync::Arc;

    #[test]
    fn range_lock_read_test() {
        let data: Vec<usize> = vec![0, 1, 2, 3, 4, 5];
        let lock = RangeLock::from(data);
        assert!(lock.data().unwrap()[0..3] == [0, 1, 2])
    }

    #[test]
    fn range_lock_write_test() {
        let data: Vec<usize> = vec![0, 1, 2, 3, 4, 5];
        let lock = RangeLock::from(data);
        if let RangeLockResult::Ok(mut guard) = lock.get_mut(0..3) {
            if let RangeLockGuard(LockGuard::Write(ref mut values), ..) = guard {
                values[0] = 2;
                values[1] = 1;
                values[2] = 0;
            }
        };
        assert!(lock.data().unwrap()[0..3] == [2, 1, 0])
    }

    #[test]
    fn conflict_write_test() {
        let data: Vec<usize> = vec![0, 1, 2, 3, 4, 5];
        let lock = Arc::new(RangeLock::from(data));
        let lock1 = lock.clone();
        let lock2 = lock.clone();
        let _a_read = lock1.get(0..3);
        if let RangeLockResult::RangeConflict = lock2.get_mut(1..2) {
            assert!(true);
        } else {
            assert!(false)
        };
    }

    #[test]
    fn no_conflict_write_test() {
        let data: Vec<usize> = vec![0, 1, 2, 3, 4, 5];
        let lock = Arc::new(RangeLock::from(data));
        let lock1 = lock.clone();
        let lock2 = lock.clone();
        let _a_read = lock1.get_mut(0..2);
        if let RangeLockResult::Ok(..) = lock2.get_mut(3..4) {
            assert!(true);
        } else {
            assert!(false)
        };
    }

    #[test]
    fn no_conflict_read_test() {
        let data: Vec<usize> = vec![0, 1, 2, 3, 4, 5];
        let lock = Arc::new(RangeLock::from(data));
        let lock1 = lock.clone();
        let lock2 = lock.clone();
        let _a_read = lock1.get(0..3);
        if let RangeLockResult::Ok(..) = lock2.get(1..2) {
            assert!(true);
        } else {
            assert!(false)
        };
    }

    #[test]
    fn conflict_read_test() {
        let data: Vec<usize> = vec![0, 1, 2, 3, 4, 5];
        let lock = Arc::new(RangeLock::from(data));
        let lock1 = lock.clone();
        let lock2 = lock.clone();
        let _a_read = lock1.get_mut(0..3);
        if let RangeLockResult::RangeConflict = lock2.get(2..4) {
            assert!(true);
        } else {
            assert!(false)
        };
    }

    #[test]
    fn range_limit_test() {
        let data: Vec<usize> = vec![0, 1, 2, 3, 4, 5];
        let lock = Arc::new(RangeLock::from(data));
        if let RangeLockResult::BadRange  = lock.get_mut(6..9) {
            assert!(true);
        } else {
            assert!(false)
        };
    }
}
