/// M01 runtime integer cell. Its signed 16-bit range is part of the M01 contract.
pub type Cell = i16;

#[cfg(test)]
mod tests {
    use super::Cell;

    #[test]
    fn signed_sixteen_bit_limits() {
        assert_eq!(Cell::MIN, -32768);
        assert_eq!(Cell::MAX, 32767);
    }
}
