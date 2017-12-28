#[macro_use]
extern crate derive_new;
#[macro_use]
extern crate derive_is_enum_variant;

extern crate may;
use may::coroutine;
use may::config;

use std::sync::Arc;
use std::fmt::Debug;
use std::sync::Mutex;
use std::sync::MutexGuard;
use std::sync::PoisonError;
use std::ops::Range;
use std::cell::UnsafeCell;
use std::thread;

// TODO: add some bounds checks on the range
// TODO: make the ranges an interval tree and perhaps use split_at(s)

fn overlaps(x: &Range<usize>, y: &Range<usize>) -> bool {
    x.start <= y.end && y.start <= x.end
}


// aliases for different ranges
type Ranges = Vec<RangeType>;
type GuardedRanges<'a> = MutexGuard<'a, Ranges>;

/// # RangeType
/// The type of range.
#[derive(is_enum_variant)]
#[derive(PartialEq, Clone, Debug)]
enum RangeType {
    Read(Range<usize>),
    Write(Range<usize>),
}

impl RangeType {
    /// get the inner range of the range type
    fn get(&self) -> &Range<usize> {
        match *self {
            RangeType::Read(ref r1) => &r1,
            RangeType::Write(ref r2) => &r2,
        }
    }
    /// test if the range overlaps with another
    fn overlaps(&self, other: &Self) -> bool {
        let r1 = self.get();
        let r2 = other.get();
        overlaps(&r1, &r2)
    }
}

/// # LockRange
/// A lock over a range of value [T]. Drop the range from the lock
/// when the LockedRange goes out of scope.
#[derive(new)]
struct LockRange<'lock, T: 'lock + Copy + Debug> {
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
enum LockGuard<'lock, T: 'lock + Copy + Debug> {
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
struct RangeLockGuard<'lock, T: 'lock + Copy + Debug>(LockGuard<'lock, T>, LockRange<'lock, T>);

/// # RangeLockResult
/// The result of a range locking attempt
enum RangeLockResult<'lock, T: 'lock + Copy + Debug> {
    Ok(RangeLockGuard<'lock, T>),
    RangeConflict,
    OtherError,
}

/// # RangeLock
/// Allows multiple immutable and mutable borrows based on access ranges.
struct RangeLock<T: Copy + Debug> {
    ranges: Mutex<Vec<RangeType>>,
    data: UnsafeCell<Vec<T>>,
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
            ranges: Mutex::new(vec![]),
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
    fn get_mut(&self, range: Range<usize>) -> RangeLockResult<T> {
        // get a typed range
        let trange = RangeType::Write(range.clone());
        // evaluate whether or not the range can be accessed
        if let Ok(mut ranges) = self.lock() {
            // check for any conflicts with existing reads and writes
            if ranges.iter().any(|ref existing| existing.overlaps(&trange)) {
                return RangeLockResult::RangeConflict;
            }
            // add the range to the set of ranges
            ranges.push(trange.clone());
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
    fn get(&self, range: Range<usize>) -> RangeLockResult<T> {
        // get a typed range
        let trange = RangeType::Read(range.clone());
        // evaluate whether or not the range can be accessed
        if let Ok(mut ranges) = self.lock() {
            // check for any conflicts with existing writes
            if ranges.iter().filter(|ref rt| rt.is_write()).any(
                |ref existing| existing.overlaps(&trange),
            )
            {
                return RangeLockResult::RangeConflict;
            }
            // add the range to the set of ranges
            ranges.push(trange.clone());
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
            if let Some(idx) = ranges.iter().position(|e| e == range) {
                println!("remove({:?})", range);
                ranges.remove(idx);
            }
        }
    }
}


fn main() {

    // set number of workers for coroutines (i.e. 1 == sequential)
    let c = config();
    c.set_workers(1);

    // make an array of data
    let data = Arc::new(RangeLock::from(vec![0u32; 100]));

    // ranges
    let ranges = vec![0..10, 5..20, 21..31, 30..41, 42..44, 45..46, 70..78, 77..80];

    // spawn 4 writers
    let mut handles = vec![];
    for (idx, range) in ranges.iter().enumerate() {

        // set up localized data values
        let local = data.clone();
        let range = range.clone();
        let idx = idx.clone();

        // spawn the coroutine or thread
        handles.push(coroutine::spawn(move || {

            println!("thread[{}] hello", idx);
            match local.get_mut(range) {
                RangeLockResult::Ok(mut guard) => {
                    if let RangeLockGuard(LockGuard::Write(ref mut values), ..) = guard {
                        println!("thread[{}] values = [{:?}]", idx, values);
                        values[0] = 1000;
                    }
                }
                RangeLockResult::RangeConflict => {
                    println!("thread[{}] RANGE CONFLICT", idx);
                }
                RangeLockResult::OtherError => {
                    println!("thread[{}] OTHER ERROR", idx);
                }
            };
        }));
    }

    // join all the handles to the main
    for h in handles.into_iter() {
        if !h.join().is_ok() {
            println!("THREAD JOIN ERROR!");
        }
    }

    // print the data
    println!("thread[FINAL] goodbye");
    match data.get(0..100) {
        RangeLockResult::Ok(guard) => {
            if let RangeLockGuard(LockGuard::Read(ref values), ..) = guard {
                println!("thread[FINAL] values = [{:?}]", values);
            }
        }
        RangeLockResult::RangeConflict => {
            println!("thread[FINAL] RANGE CONFLICT");
        }
        RangeLockResult::OtherError => {
            println!("thread[FINAL] OTHER ERROR");
        }
    };
}
