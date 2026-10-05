use octowhere_peripherals::touch::{TouchData, TouchPoint};
type TouchRead = Result<TouchData, ()>;
fn is_report(read: &TouchRead) -> bool {
    matches!(read, Ok(TouchData::Points(_) | TouchData::Lifted(_) | TouchData::Gesture(_) | TouchData::CoverGesture))
}
fn is_contact(read: &TouchRead) -> bool { matches!(read, Ok(TouchData::Points(points)) if !points.is_empty()) }
// Copied from firmware/src/main.rs put_touch_read, the lock removed.
fn put(reads: &mut heapless::Deque<TouchRead, 2>, read: TouchRead) {
    let replace = match reads.back() {
        None => false,
        Some(back) if !is_report(&read) && is_report(back) => return,
        Some(back) => !is_report(&read) || is_contact(back) || !is_report(back) || reads.is_full(),
    };
    if replace && let Some(back) = reads.back_mut() { *back = read; } else { _ = reads.push_back(read); }
}
fn main() {
    let p = TouchPoint { x: 1, y: 2 };
    let mut contact = heapless::Vec::new(); contact.push(p).unwrap();
    let mut q = heapless::Deque::<TouchRead, 2>::new();
    put(&mut q, Ok(TouchData::Lifted(p)));
    put(&mut q, Ok(TouchData::CoverGesture));
    put(&mut q, Ok(TouchData::Points(contact)));
    println!("{:?}", q.iter().collect::<Vec<_>>());
}
