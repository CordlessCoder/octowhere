use embedded_hal_async::i2c::{ErrorType, I2c, Operation};
use octowhere_peripherals::rtc::Pcf85063aRtc;
#[derive(Debug)] struct E;
impl embedded_hal::i2c::Error for E { fn kind(&self) -> embedded_hal::i2c::ErrorKind { embedded_hal::i2c::ErrorKind::Other } }
struct Fake;
impl ErrorType for Fake { type Error = E; }
impl I2c for Fake {
    async fn transaction(&mut self, _a: u8, ops: &mut [Operation<'_>]) -> Result<(), E> {
        for op in ops { if let Operation::Read(buf) = op {
            // 12:00:00, day 1, weekday 3, month 1, year register 0xAB (not BCD)
            buf.copy_from_slice(&[0x00, 0x00, 0x12, 0x01, 0x03, 0x01, 0xAB][..buf.len()]);
        } }
        Ok(())
    }
}
fn main() {
    let mut rtc = Pcf85063aRtc::new(Fake);
    let waker = std::task::Waker::noop();
    let mut cx = std::task::Context::from_waker(waker);
    let mut fut = std::pin::pin!(rtc.get_time());
    match fut.as_mut().poll(&mut cx) {
        std::task::Poll::Ready(r) => println!("{:?}", r),
        _ => println!("pending"),
    }
}
