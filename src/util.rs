const CHUNK_SIZE: usize = 32;
#[inline]
pub fn fill_buf_repeat<'b>(mut buf: &'b mut [u8], mut data: &[u8], mut n: usize) -> &'b mut [u8] {
    if n == 0 {
        return &mut [];
    }
    let bytes = data.len() * n;
    assert!(buf.len() >= bytes);
    if bytes >= 4 * CHUNK_SIZE && CHUNK_SIZE.is_multiple_of(data.len()) {
        let mut arr = [0; CHUNK_SIZE];
        arr.chunks_exact_mut(data.len())
            .for_each(|chunk| chunk.copy_from_slice(data));
        let mut chunks = buf.chunks_exact_mut(arr.len());
        chunks
            .by_ref()
            .take(n / (CHUNK_SIZE / data.len()))
            .for_each(|chunk| {
                chunk.copy_from_slice(&arr);
            });
        n %= CHUNK_SIZE / data.len();
        buf = chunks.into_remainder();
    }
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
