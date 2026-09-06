//! Spreading independent work over the machine's cores.
//!
//! A media library is a lot of folders that have nothing to say to each other:
//! reading them, working out their names, and deciding their renames are all
//! per-folder jobs. This module is the small amount of shared machinery that
//! lets those jobs run at once, without pulling in a dependency for it.
//!
//! Everything here falls back to running in the caller's thread when there is
//! too little work to be worth a thread, so behaviour never depends on how many
//! cores are available — only on how long it takes.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread;

/// Past this many threads, more of them stop paying for themselves: the work
/// below is a mixture of directory reads and short bursts of matching.
const MAX_WORKERS: usize = 16;

/// Below this many items, the threads cost more than they save.
const MIN_ITEMS_PER_WORKER: usize = 4;

/// How many threads to spend on `items` pieces of work; 1 means "stay here".
pub fn worker_count(items: usize) -> usize {
    if items < MIN_ITEMS_PER_WORKER * 2 {
        return 1;
    }
    let cores = thread::available_parallelism().map_or(1, |cores| cores.get());
    cores
        .clamp(1, MAX_WORKERS)
        .min(items / MIN_ITEMS_PER_WORKER)
}

/// Apply `f` to every item, in parallel, returning the results in input order.
///
/// The order is part of the contract: a plan has to read the same way whichever
/// thread happened to finish first.
pub fn map<T, R>(items: &[T], f: impl Fn(&T) -> R + Sync) -> Vec<R>
where
    T: Sync,
    R: Send,
{
    map_with(items, || (), |(), item| f(item))
}

/// [`map`], giving each worker a piece of scratch state of its own.
///
/// `state` is called once per thread, not once per item, which is what makes a
/// reusable buffer worth having.
pub fn map_with<T, S, R>(
    items: &[T],
    state: impl Fn() -> S + Sync,
    f: impl Fn(&mut S, &T) -> R + Sync,
) -> Vec<R>
where
    T: Sync,
    R: Send,
{
    let workers = worker_count(items.len());
    if workers <= 1 {
        let mut state = state();
        return items.iter().map(|item| f(&mut state, item)).collect();
    }

    let next = AtomicUsize::new(0);
    let claim = || {
        let mut state = state();
        let mut mine: Vec<(usize, R)> = Vec::new();
        loop {
            let index = next.fetch_add(1, Ordering::Relaxed);
            let Some(item) = items.get(index) else {
                return mine;
            };
            mine.push((index, f(&mut state, item)));
        }
    };

    let mut results: Vec<(usize, R)> = thread::scope(|scope| {
        let handles: Vec<_> = (0..workers).map(|_| scope.spawn(claim)).collect();
        handles.into_iter().flat_map(join).collect()
    });
    results.sort_by_key(|(index, _)| *index);
    results.into_iter().map(|(_, result)| result).collect()
}

/// Wait for a worker, carrying a panic on to the caller rather than quietly
/// returning half a result.
fn join<R>(handle: thread::ScopedJoinHandle<'_, R>) -> R {
    handle
        .join()
        .unwrap_or_else(|payload| std::panic::resume_unwind(payload))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_results_in_input_order() {
        let items: Vec<usize> = (0..1000).collect();
        let doubled = map(&items, |item| item * 2);
        assert_eq!(
            doubled,
            items.iter().map(|item| item * 2).collect::<Vec<_>>()
        );
    }

    #[test]
    fn handles_a_workload_too_small_to_spread() {
        assert_eq!(map(&[1, 2, 3], |item| item + 1), [2, 3, 4]);
        assert!(map::<u8, u8>(&[], |item| *item).is_empty());
    }
}
