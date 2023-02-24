use nom::error::{ErrorKind, ParseError};
use nom::Err;
use nom::IResult;

pub fn be_u64_varint(i: &[u8]) -> IResult<&[u8], u64> {
    let mut res = 0;
    // to guard from overflow
    let max_slice = &i[0..(i.len().min(5))];
    for (id, &b) in max_slice.iter().enumerate() {
        let b = b as u64;
        res = (res << 7) | (b & 0b0111_1111);

        if b >> 7 == 0 {
            return Ok((&i[id + 1..], res));
        }
    }

    Err(Err::Error(ParseError::from_error_kind(
        i,
        ErrorKind::MapOpt,
    )))
}

#[cfg(test)]
mod tests {
    use crate::varint::be_u64_varint;

    #[test]
    fn parse_1_byte() {
        let varint = [0b0000_1111];
        let (i, res) = be_u64_varint(&varint).unwrap();

        assert!(i.is_empty());
        assert_eq!(res, 0b0000_1111);
    }

    #[test]
    fn parse_2_byte() {
        let varint = [0b1000_1111, 0b0000_1011];
        let (i, res) = be_u64_varint(&varint).unwrap();

        assert!(i.is_empty());
        assert_eq!(res, 0b1111_000_1011);
    }

    #[test]
    fn parse_3_byte() {
        let varint = [0b1000_1111, 0b1000_1101, 0b0000_1011];
        let (i, res) = be_u64_varint(&varint).unwrap();

        assert!(i.is_empty());
        assert_eq!(res, 0b1111_000_1101_000_1011);
    }

    #[test]
    fn parse_4_byte() {
        let varint = [0b1000_1111, 0b1000_0111, 0b1000_1101, 0b0000_1011];
        let (i, res) = be_u64_varint(&varint).unwrap();

        assert!(i.is_empty());
        assert_eq!(res, 0b1111_000_0111_000_1101_000_1011);
    }

    #[test]
    fn parse_5_byte() {
        let varint = [
            0b1000_1111,
            0b1000_1110,
            0b1000_0111,
            0b1000_1101,
            0b0000_1011,
        ];
        let (i, res) = be_u64_varint(&varint).unwrap();

        assert!(i.is_empty());
        assert_eq!(res, 0b1111_000_1110_000_0111_000_1101_000_1011);
    }

    #[test]
    fn ignore_rest() {
        let varint = [
            0b1000_1111,
            0b1000_1110,
            0b1000_0111,
            0b1000_1101,
            0b0000_1011,
            0b0,
        ];
        let (i, _) = be_u64_varint(&varint).unwrap();

        assert_eq!(i.len(), 1);
    }
}
