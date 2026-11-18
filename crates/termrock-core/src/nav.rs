pub fn step_index(cursor: usize, len: usize, delta: isize, wrap: bool) -> usize {
    if len == 0 {
        return 0;
    }
    let to = cursor as isize + delta;
    if wrap {
        to.rem_euclid(len as isize) as usize
    } else {
        to.clamp(0, len as isize - 1) as usize
    }
}
