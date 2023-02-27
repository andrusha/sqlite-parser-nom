use nom::number::complete::{be_u16, be_u32};
use nom::sequence::pair;
use nom::IResult;

/// Big-ending signed 48-bit two-complimentary integer
pub fn be_i48(i: &[u8]) -> IResult<&[u8], i64> {
    let (i, (head, tail)) = pair(be_u16, be_u32)(i)?;
    let mut x = (head as u64) << 32 | (tail as u64);
    if x & 0x80_00_00_00_00_00 != 0 {
        x |= 0xff_ff_00_00_00_00_00_00;
    };

    Ok((i, x as i64))
}

#[cfg(test)]
mod tests {
    use crate::be_i48::be_i48;

    #[test]
    fn consumes_6_bytes() {
        let bytes = [0x11, 0x22, 0x33, 0x44, 0x55, 0x66];
        let (i, res) = be_i48(&bytes).unwrap();

        assert_eq!(i.len(), 0); // consumes all
        assert_eq!(res, 0x11_22_33_44_55_66);
    }

    #[test]
    fn fails_on_short_input() {
        let bytes = [0x11, 0x22, 0x33, 0x44, 0x55];
        let res = be_i48(&bytes);

        assert!(res.is_err());
    }

    #[test]
    fn passes_through_rest() {
        let bytes = [0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77];
        let (i, _) = be_i48(&bytes).unwrap();

        assert_eq!(i.len(), 1);
        assert_eq!(i.first().unwrap().to_owned(), 0x77);
    }
}
