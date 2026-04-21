pub fn fill_buf_repeat<'b>(buf: &'b mut [u8], data: &[u8], n: usize) -> &'b mut [u8] {
    if n == 0 {
        return &mut [];
    }
    assert!(buf.len() >= data.len() * n);
    // `2^expn` repetition is done by doubling `buf` `expn`-times.
    buf[..data.len()].copy_from_slice(data);
    let mut len = data.len();
    {
        let mut m = n >> 1;
        // If `m > 0`, there are remaining bits up to the leftmost '1'.
        while m > 0 {
            // `buf.extend(buf)`:
            unsafe {
                core::ptr::copy_nonoverlapping(buf.as_ptr(), (buf.as_mut_ptr()).add(len), len);
                len *= 2;
            }

            m >>= 1;
        }
    }

    // `rem` (`= n - 2^expn`) repetition is done by copying
    // first `rem` repetitions from `buf` itself.
    let rem_len = n * data.len() - len;
    if rem_len > 0 {
        // `buf.extend(buf[0 .. rem_len])`:
        unsafe {
            // This is non-overlapping since `2^expn > rem`.
            core::ptr::copy_nonoverlapping(buf.as_ptr(), (buf.as_mut_ptr()).add(len), rem_len);
        }
    }
    &mut buf[..data.len() * n]
}
