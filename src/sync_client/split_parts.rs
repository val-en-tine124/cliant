
use std::cmp::min;

/// split a given file size to sizes that can add up to the original size.
pub fn split_parts(
    file_size_bytes: u64,
    max_concurrent_part: u32,
    min_split_part_mb: u32,
) -> Vec<Vec<u64>> {
    if file_size_bytes == 0 {
        return vec![];
    }
    let min_split_part_bytes = min_split_part_mb as u64 * 1024 * 1024;

    // Edge case, if file size is less than or  equal to the minimum split bytes
    if file_size_bytes <= min_split_part_bytes {
        return vec![vec![0, file_size_bytes - 1]];
    }
    if min_split_part_bytes == 0 {
        // Edge case, incase max_concurrent_split is equal to zero
        return vec![vec![0, file_size_bytes - 1]];
    }

    let num_part_ideal = file_size_bytes / min_split_part_bytes;
    let num_parts = min(
        if num_part_ideal > 0 {
            num_part_ideal as u32
        } else {
            1
        },
        max_concurrent_part,
    );
    let chunk_bytes = file_size_bytes / num_parts as u64;
    let remaining_chunk = file_size_bytes % num_parts as u64;
    let mut parts: Vec<Vec<u64>> = Vec::new();
    let mut current_start: u64 = 0;
    for i in 0..num_parts {
        // iterate over number of parts
        let mut current_end = current_start + chunk_bytes - 1;
        if i < remaining_chunk as u32 {
            current_end += 1; // make sure all chunk is distributed through out the bytes segments
        }
        parts.push(vec![current_start, current_end]); //  save current state
        current_start = current_end + 1; // update current_start for next iteration.
    }
    parts
}
