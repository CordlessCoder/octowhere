pub fn fill_buf_repeat<'b>(buf: &'b mut [u8], data: &[u8], n: usize) -> &'b mut [u8] {
    if n == 0 {
        return &mut [];
    }
    assert!(buf.len() >= data.len() * n);
    buf.chunks_exact_mut(data.len()).take(n).for_each(|chunk| {
        chunk.copy_from_slice(data);
    });

    &mut buf[..data.len() * n]
}
