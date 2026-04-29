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

pub fn widening_copy<const FACTOR: usize>(buf: &mut [u8], data: &[u8], width: usize) {
    const {
        assert!(FACTOR != 0);
    }
    if const { FACTOR == 1 } {
        buf.copy_from_slice(data);
    } else {
        buf.chunks_exact_mut(width * FACTOR)
            .zip(data.chunks_exact(width))
            .for_each(|(chunk, source)| {
                fill_buf_repeat(chunk, source, FACTOR);
            });
    }
}
