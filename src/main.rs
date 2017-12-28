extern crate may;
extern crate range_lock as lock;

use lock::*;

use may::coroutine;
use may::config;

use std::thread;
use std::sync::Arc;

fn using_coroutines(workers: usize) {

    // set number of workers for coroutines (i.e. 1 == sequential)
    let c = config();
    c.set_workers(workers);

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

            println!("worker[{}] hello", idx);
            match local.get_mut(range) {
                RangeLockResult::Ok(mut guard) => {
                    if let RangeLockGuard(LockGuard::Write(ref mut values), ..) = guard {
                        println!("worker[{}] values = [{:?}]", idx, values);
                        values[0] = 1000;
                    }
                }
                RangeLockResult::RangeConflict => {
                    println!("worker[{}] RANGE CONFLICT", idx);
                }
                RangeLockResult::OtherError => {
                    println!("worker[{}] OTHER ERROR", idx);
                }
            };
        }));
    }

    // join all the handles to the main
    for h in handles.into_iter() {
        if !h.join().is_ok() {
            println!("WORKER JOIN ERROR!");
        }
    }

    // print the data
    println!("thread[FINAL] goodbye");
    match data.get(0..100) {
        RangeLockResult::Ok(guard) => {
            if let RangeLockGuard(LockGuard::Read(ref values), ..) = guard {
                println!("worker[FINAL] values = [{:?}]", values);
            }
        }
        RangeLockResult::RangeConflict => {
            println!("worker[FINAL] RANGE CONFLICT");
        }
        RangeLockResult::OtherError => {
            println!("worker[FINAL] OTHER ERROR");
        }
    };
}

fn using_std_threads() {
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
        handles.push(thread::spawn(move || {

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

fn main() {

    // exmaple using coroutines from may crate
    //using_coroutines(1); // uncomment for sequential and then --|
    using_coroutines(8); //     comment out  <--------------------

    // example using threads
    using_std_threads();

}
