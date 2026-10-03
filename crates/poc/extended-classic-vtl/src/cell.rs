use crate::executable::RuntimeError;

pub type Cell = i16;

pub(crate) fn add(lhs: Cell, rhs: Cell) -> Cell {
    lhs.wrapping_add(rhs)
}

pub(crate) fn sub(lhs: Cell, rhs: Cell) -> Cell {
    lhs.wrapping_sub(rhs)
}

pub(crate) fn mul(lhs: Cell, rhs: Cell) -> Cell {
    lhs.wrapping_mul(rhs)
}

pub(crate) fn div(lhs: Cell, rhs: Cell) -> Result<Cell, RuntimeError> {
    if rhs == 0 {
        return Err(RuntimeError::DivisionByZero);
    }
    if lhs == Cell::MIN && rhs == -1 {
        return Ok(Cell::MIN);
    }
    Ok(lhs / rhs)
}

pub(crate) fn rem(lhs: Cell, rhs: Cell) -> Result<Cell, RuntimeError> {
    if rhs == 0 {
        return Err(RuntimeError::RemainderByZero);
    }
    if lhs == Cell::MIN && rhs == -1 {
        return Ok(0);
    }
    Ok(lhs % rhs)
}

pub(crate) fn eq(lhs: Cell, rhs: Cell) -> Cell {
    if lhs == rhs { 1 } else { 0 }
}

pub(crate) fn ne(lhs: Cell, rhs: Cell) -> Cell {
    if lhs != rhs { 1 } else { 0 }
}

pub(crate) fn lt(lhs: Cell, rhs: Cell) -> Cell {
    if lhs < rhs { 1 } else { 0 }
}

pub(crate) fn le(lhs: Cell, rhs: Cell) -> Cell {
    if lhs <= rhs { 1 } else { 0 }
}

pub(crate) fn gt(lhs: Cell, rhs: Cell) -> Cell {
    if lhs > rhs { 1 } else { 0 }
}

pub(crate) fn ge(lhs: Cell, rhs: Cell) -> Cell {
    if lhs >= rhs { 1 } else { 0 }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arithmetic_wraps_to_16_bits() {
        assert_eq!(add(Cell::MAX, 1), Cell::MIN);
        assert_eq!(sub(Cell::MIN, 1), Cell::MAX);
        assert_eq!(mul(30_000, 3), 24_464);
    }

    #[test]
    fn signed_division_and_remainder_follow_contract() {
        assert_eq!(div(-7, 3), Ok(-2));
        assert_eq!(div(7, -3), Ok(-2));
        assert_eq!(rem(-7, 3), Ok(-1));
        assert_eq!(rem(7, -3), Ok(1));
        assert_eq!(div(Cell::MIN, -1), Ok(Cell::MIN));
        assert_eq!(rem(Cell::MIN, -1), Ok(0));
        assert_eq!(div(1, 0), Err(RuntimeError::DivisionByZero));
        assert_eq!(rem(1, 0), Err(RuntimeError::RemainderByZero));
    }

    #[test]
    fn comparisons_are_normalized() {
        assert_eq!(eq(2, 2), 1);
        assert_eq!(ne(2, 2), 0);
        assert_eq!(lt(-1, 0), 1);
        assert_eq!(le(1, 0), 0);
        assert_eq!(gt(4, 3), 1);
        assert_eq!(ge(3, 4), 0);
    }
}
