//! # Split Parts
//!
//! This module contains the logic for splitting a file into multiple parts for
//! parallel downloading.

use std::cmp::min;

/// Splits a given file size into a vector of byte ranges.
///
/// This function takes the total file size, the maximum number of concurrent
/// parts, and the minimum size of each part in megabytes, and returns a vector
/// of byte ranges that can be downloaded in parallel.
///
/// # Arguments
///
/// * `file_size_bytes` - The total size of the file in bytes.
/// * `max_concurrent_part` - The maximum number of concurrent parts to download.
/// * `min_split_part_mb` - The minimum size of each part in megabytes.
///
/// # Returns
///
/// A vector of vectors, where each inner vector represents a byte range with a
/// start and end offset.
pub fn split_parts(
    file_size_bytes: usize,
    max_concurrent_part: usize,
    min_split_part_mb: usize,
) -> Vec<[usize;2]> {
    if file_size_bytes == 0 {
        return vec![];
    }

    let min_split_part_bytes = min_split_part_mb  * 1024 * 1024;

    // Edge case, if file size is less than or  equal to the minimum split bytes
    if file_size_bytes <= min_split_part_bytes {
        return vec![[0, file_size_bytes - 1]];
    }
    if min_split_part_bytes == 0 {
        // Edge case, incase max_concurrent_split is equal to zero
        return vec![[0, file_size_bytes - 1]];
    }

    if max_concurrent_part == 0 {
        //Incase user max_concurrent_part input is zero
        return vec![[0, file_size_bytes - 1]];
    }

    let num_part_ideal = file_size_bytes / min_split_part_bytes;
    let num_parts = min(
        if num_part_ideal > 0 {
            num_part_ideal
        } else {
            1
        },
        max_concurrent_part,
    );
    let chunk_bytes = file_size_bytes / num_parts;
    let remaining_chunk = file_size_bytes % num_parts;
    let mut parts: Vec<[usize;2]> = Vec::new();
    let mut current_start = 0;
    for i in 0..num_parts {
        // iterate over number of parts
        let mut current_end = current_start + chunk_bytes - 1;
        if i < remaining_chunk {
            current_end += 1; // make sure all chunk is distributed through out the bytes segments
        }
        parts.push([current_start, current_end]); //  save current state
        current_start = current_end + 1; // update current_start for next iteration.
    }
    parts
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_split_parts_zero_file_size() {
        let parts = split_parts(0, 10, 1);
        assert!(parts.is_empty());
    }

    #[test]
    fn test_split_parts_small_file_less_than_min_part() {
        let parts = split_parts(5 * 1024 * 1024, 10, 10); // 5MB file, 10MB min part
        assert_eq!(parts, vec![[0, 5 * 1024 * 1024 - 1]]);
    }

    #[test]
    fn test_split_parts_small_file_equal_to_min_part() {
        let parts = split_parts(10 * 1024 * 1024, 10, 10); // 10MB file, 10MB min part
        assert_eq!(parts, vec![[0, 10 * 1024 * 1024 - 1]]);
    }

    #[test]
    fn test_split_parts_min_split_part_mb_zero() {
        let parts = split_parts(100, 10, 0); // 100 bytes file, 0MB min part
        assert_eq!(parts, vec![[0, 99]]);
    }



    #[test]
    fn test_split_parts_more_concurrent_than_ideal() {
        let file_size = 10 * 1024 * 1024; // 10MB
        let max_concurrent = 20;
        let min_split = 1; // 1MB
        let parts = split_parts(file_size, max_concurrent, min_split);
        assert_eq!(parts.len(), 10); // Should still be 10 parts of 1MB
        assert_eq!(parts[0], [0, 1 * 1024 * 1024 - 1]);
        assert_eq!(parts[9], [9 * 1024 * 1024, 10 * 1024 * 1024 - 1]);
    }

    #[test]
    fn test_split_parts_fewer_concurrent_than_ideal() {
        let file_size = 100 * 1024 * 1024; // 100MB
        let max_concurrent = 5;
        let min_split = 1; // 1MB
        let parts = split_parts(file_size, max_concurrent, min_split);
        assert_eq!(parts.len(), 5); // Should be limited by max_concurrent
        assert_eq!(parts[0], [0, 20 * 1024 * 1024 - 1]);
        assert_eq!(parts[4], [80 * 1024 * 1024, 100 * 1024 * 1024 - 1]);
    }

    #[test]
    fn test_split_parts_single_part_due_to_max_concurrent() {
        let file_size = 100 * 1024 * 1024; // 100MB
        let max_concurrent = 1;
        let min_split = 1; // 1MB
        let parts = split_parts(file_size, max_concurrent, min_split);
        assert_eq!(parts.len(), 1);
        assert_eq!(parts[0], [0, 100 * 1024 * 1024 - 1]);
    }
}

