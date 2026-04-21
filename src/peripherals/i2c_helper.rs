// Async I2C helpers that can be used by every I2C device, allowing them to reuse the same async
// state machine.

use embedded_hal_async::i2c::I2c;

pub enum I2cOperation {
    WriteReg(u8, u8),
}

pub async fn batch<I: I2c>(i2c: &mut I, addr: u8, ops: &[I2cOperation]) -> Result<(), I::Error> {
    for op in ops {
        execute(i2c, addr, op).await?;
    }
    Ok(())
}

pub async fn execute<I: I2c>(i2c: &mut I, addr: u8, op: &I2cOperation) -> Result<(), I::Error> {
    match op {
        &I2cOperation::WriteReg(reg, val) => write_reg(i2c, addr, reg, val).await,
    }
}

pub async fn read_reg_byte<I: I2c>(i2c: &mut I, addr: u8, reg: u8) -> Result<u8, I::Error> {
    let mut buf = [0u8];
    read_reg(i2c, addr, reg, &mut buf).await?;
    Ok(buf[0])
}

pub async fn read_reg<I: I2c>(
    i2c: &mut I,
    addr: u8,
    reg: u8,
    buf: &mut [u8],
) -> Result<(), I::Error> {
    i2c.write_read(addr, &[reg], buf).await
}

pub async fn write_reg<I: I2c>(i2c: &mut I, addr: u8, reg: u8, val: u8) -> Result<(), I::Error> {
    i2c.write(addr, &[reg, val]).await
}

pub async fn write_wide_reg<I: I2c>(
    i2c: &mut I,
    addr: u8,
    reg: u16,
    val: u8,
) -> Result<(), I::Error> {
    let reg = reg.to_be_bytes();
    i2c.write(addr, &[reg[0], reg[1], val]).await
}
