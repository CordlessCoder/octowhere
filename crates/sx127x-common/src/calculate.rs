pub fn frf(hz: u32, fstep: f32) -> u32 {
    ((hz as f32) / fstep) as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frf_ok() {
        let res = frf(434_000_000, (32_000_000f32) / (2u32.pow(19) as f32));
        assert_eq!(res, 0x6c8000);
    }
}
