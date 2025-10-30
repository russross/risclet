#[cfg(test)]
mod tests {
    use crate::ast::{CompressedOp, CompressedOperands, Location};
    use crate::encoder_compressed::encode_compressed_inst;

    #[test]
    fn test_c_nop_encoding() {
        // c.nop: 000 | 0 | 00000 | 00000 | 01 = 0x0001
        let location = Location { file: "test.s".to_string(), line: 1 };
        let result = encode_compressed_inst(
            &CompressedOp::CNop,
            &CompressedOperands::None,
            &location,
            None,
        );
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 0x0001);
    }

    #[test]
    fn test_c_ebreak_encoding() {
        // c.ebreak: 1001 | 00000 | 00000 | 10 = 0x9002
        let location = Location { file: "test.s".to_string(), line: 1 };
        let result = encode_compressed_inst(
            &CompressedOp::CEbreak,
            &CompressedOperands::None,
            &location,
            None,
        );
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 0x9002);
    }
}
